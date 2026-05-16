/*
Maddy Mail Server - Composable all-in-one email server.
Copyright © 2019-2020 Max Mazurov <fox.cpp@disroot.org>, Maddy Mail Server contributors

This program is free software: you can redistribute it and/or modify
it under the terms of the GNU General Public License as published by
the Free Software Foundation, either version 3 of the License, or
(at your option) any later version.

This program is distributed in the hope that it will be useful,
but WITHOUT ANY WARRANTY; without even the implied warranty of
MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
GNU General Public License for more details.

You should have received a copy of the GNU General Public License
along with this program.  If not, see <https://www.gnu.org/licenses/>.
*/

package pgp_verify

import (
	"io"
	"mime"
	"net/mail"
	"strings"

	"github.com/emersion/go-message/textproto"
	"github.com/themadorg/madmail/framework/exterrors"
)

// Policy is the full PGP-only gate: envelope/header rules plus body
// structure validation. Prefer this over bare EnforceEncryption when
// SMTP context is available (submission, check.pgp_encryption).
type Policy struct {
	MailFrom   string
	Recipients []string

	PassthroughSenders    []string
	PassthroughRecipients []string

	// AllowSecureJoin permits unencrypted multipart/mixed Secure-Join
	// handshake messages. When false, Secure-Join headers are ignored.
	AllowSecureJoin bool

	// RequireFromMatchesEnvelope rejects when MIME From != MAIL FROM
	// (mailer-daemon bounces excepted). Used on submission.
	RequireFromMatchesEnvelope bool

	// ValidateRecipientFormat rejects RCPT with other than one @.
	ValidateRecipientFormat bool
}

// PolicyFromOptions maps the legacy Options struct to Policy with
// strict defaults for body-only callers (mxdeliv, IMAP, etc.).
func PolicyFromOptions(o Options) Policy {
	return Policy{
		MailFrom:              o.MailFrom,
		Recipients:            o.Recipients,
		PassthroughSenders:    o.PassthroughSenders,
		PassthroughRecipients: o.PassthroughRecipients,
		AllowSecureJoin:       true,
	}
}

// StrictSubmissionPolicy is the default Chatmail submission policy at
// SMTP DATA: encrypted body required, From must match envelope, RCPT
// format checked. Passthrough lists are empty unless wired from config.
func StrictSubmissionPolicy(mailFrom string, recipients []string) Policy {
	return Policy{
		MailFrom:                   mailFrom,
		Recipients:                 recipients,
		AllowSecureJoin:            true,
		RequireFromMatchesEnvelope: true,
		ValidateRecipientFormat:    true,
	}
}

// SubmissionPolicy is StrictSubmissionPolicy plus install/operator
// passthrough lists and allow_secure_join (wired on the submission endpoint).
func SubmissionPolicy(mailFrom string, recipients []string, passthroughSenders, passthroughRecipients []string, allowSecureJoin bool) Policy {
	p := StrictSubmissionPolicy(mailFrom, recipients)
	p.PassthroughSenders = passthroughSenders
	p.PassthroughRecipients = passthroughRecipients
	p.AllowSecureJoin = allowSecureJoin
	return p
}

// EnforcePolicy runs header/envelope checks then streams the body
// through the PGP/MIME and Secure-Join validators.
func EnforcePolicy(header textproto.Header, body io.Reader, p Policy) error {
	opts := Options{
		MailFrom:              p.MailFrom,
		Recipients:            p.Recipients,
		PassthroughSenders:    p.PassthroughSenders,
		PassthroughRecipients: p.PassthroughRecipients,
	}

	if p.MailFrom != "" && containsFold(p.PassthroughSenders, p.MailFrom) {
		return nil
	}
	if len(p.Recipients) > 0 && allRecipientsPassthrough(p.Recipients, p.PassthroughRecipients) {
		return nil
	}

	if p.ValidateRecipientFormat {
		for _, recipient := range p.Recipients {
			if strings.Count(recipient, "@") != 1 {
				return errRejectInvalidRecipient
			}
		}
	}

	if p.RequireFromMatchesEnvelope {
		if err := checkFromMatchesEnvelope(header, p.MailFrom); err != nil {
			return err
		}
	}

	effectiveHeader := header
	if !p.AllowSecureJoin {
		effectiveHeader = header.Copy()
		effectiveHeader.Del("Secure-Join")
		effectiveHeader.Del("Secure-Join-Invitenumber")
	}

	return enforceEncryptionBody(effectiveHeader, body, opts)
}

var errRejectInvalidRecipient = &exterrors.SMTPError{
	Code:         554,
	EnhancedCode: exterrors.EnhancedCode{5, 6, 0},
	Message:      "Invalid recipient address format",
	Reason:       "invalid recipient format",
}

func checkFromMatchesEnvelope(header textproto.Header, mailFrom string) error {
	// RFC 5322: when multiple authors appear in From, Sender names the
	// transmitting agent and is the address that must match MAIL FROM.
	addressToMatch := header.Get("Sender")
	if addressToMatch == "" {
		addressToMatch = header.Get("From")
	}
	if addressToMatch == "" {
		return nil
	}
	mimeFromAddr, err := mail.ParseAddress(addressToMatch)
	if err != nil {
		return errRejectInvalidFrom
	}
	autoSubmitted := strings.ToLower(strings.TrimSpace(header.Get("Auto-Submitted")))
	daemonBounce := autoSubmitted != "" && autoSubmitted != "no" &&
		strings.HasPrefix(strings.ToLower(mimeFromAddr.Address), "mailer-daemon@") &&
		strings.HasPrefix(strings.ToLower(mailFrom), "mailer-daemon@")
	if !daemonBounce && !strings.EqualFold(mimeFromAddr.Address, mailFrom) {
		return errRejectFromMismatch
	}
	return nil
}

var (
	errRejectInvalidFrom = &exterrors.SMTPError{
		Code:         554,
		EnhancedCode: exterrors.EnhancedCode{5, 6, 0},
		Message:      "Invalid From header",
		Reason:       "invalid mime from",
	}
	errRejectFromMismatch = &exterrors.SMTPError{
		Code:         554,
		EnhancedCode: exterrors.EnhancedCode{5, 6, 0},
		Message:      "From header does not match envelope sender",
		Reason:       "from mismatch",
	}
)

// enforceEncryptionBody validates Content-Type and streams the MIME body.
// opts supplies MailFrom/Recipients for bounce and passthrough (already
// handled in EnforcePolicy when called from there; still used for
// EnforceEncryption-only entry).
func enforceEncryptionBody(header textproto.Header, body io.Reader, opts Options) error {
	if opts.MailFrom != "" && containsFold(opts.PassthroughSenders, opts.MailFrom) {
		return nil
	}
	if len(opts.Recipients) > 0 && allRecipientsPassthrough(opts.Recipients, opts.PassthroughRecipients) {
		return nil
	}
	if isAllowedBounce(header, opts.MailFrom) {
		return nil
	}

	contentType := header.Get("Content-Type")
	if strings.TrimSpace(contentType) == "" {
		return errRejectUnencrypted
	}
	mediatype, params, err := mime.ParseMediaType(contentType)
	if err != nil {
		return errRejectUnencrypted
	}

	switch strings.ToLower(mediatype) {
	case "multipart/encrypted":
		if streamValidateEncryptedMIME(body, params["boundary"]) {
			return nil
		}
		return errRejectUnencrypted
	case "multipart/mixed":
		if !isSecureJoinHeader(header) {
			return errRejectUnencrypted
		}
		if streamValidateSecureJoinMIME(body, params["boundary"]) {
			return nil
		}
		return errRejectUnencrypted
	}
	return errRejectUnencrypted
}
