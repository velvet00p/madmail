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

package pgp_encryption

import (
	"context"
	"errors"

	"github.com/emersion/go-message/textproto"
	"github.com/themadorg/madmail/framework/buffer"
	"github.com/themadorg/madmail/framework/config"
	"github.com/themadorg/madmail/framework/exterrors"
	"github.com/themadorg/madmail/framework/log"
	"github.com/themadorg/madmail/framework/module"
	"github.com/themadorg/madmail/internal/pgp_verify"
	"github.com/themadorg/madmail/internal/target"
)

const modName = "check.pgp_encryption"

type Check struct {
	instName              string
	log                   log.Logger
	passthroughSenders    []string
	passthroughRecipients []string
	requireEncryption     bool
	allowSecureJoin       bool
}

func New(_, instName string, _, inlineArgs []string) (module.Module, error) {
	c := &Check{
		instName:          instName,
		log:               log.Logger{Name: modName, Debug: log.DefaultLogger.Debug},
		requireEncryption: true,
		allowSecureJoin:   true,
	}
	return c, nil
}

func (c *Check) Name() string {
	return modName
}

func (c *Check) InstanceName() string {
	return c.instName
}

func (c *Check) Init(cfg *config.Map) error {
	cfg.Bool("require_encryption", false, true, &c.requireEncryption)
	cfg.Bool("allow_secure_join", false, true, &c.allowSecureJoin)
	cfg.StringList("passthrough_senders", false, false, nil, &c.passthroughSenders)
	cfg.StringList("passthrough_recipients", false, false, nil, &c.passthroughRecipients)
	if _, err := cfg.Process(); err != nil {
		return err
	}
	return nil
}

type state struct {
	c           *Check
	msgMeta     *module.MsgMetadata
	log         log.Logger
	mailFrom    string
	mimeFrom    string
	rcptTos     []string
	secureJoin  string
	contentType string
}

func (c *Check) CheckStateForMsg(ctx context.Context, msgMeta *module.MsgMetadata) (module.CheckState, error) {
	return &state{
		c:       c,
		msgMeta: msgMeta,
		log:     target.DeliveryLogger(c.log, msgMeta),
	}, nil
}

func (s *state) CheckConnection(ctx context.Context) module.CheckResult {
	return module.CheckResult{}
}

func (s *state) CheckSender(ctx context.Context, mailFrom string) module.CheckResult {
	s.mailFrom = mailFrom
	return module.CheckResult{}
}

func (s *state) CheckRcpt(ctx context.Context, rcptTo string) module.CheckResult {
	s.rcptTos = append(s.rcptTos, rcptTo)
	return module.CheckResult{}
}

// CheckBody defers the PGP-only policy decision to
// pgp_verify.EnforceEncryption — the single function that every
// message-accepting surface of madmail shares (SMTP submission, SMTP
// relay-in, HTTP MX-Deliv, IMAP APPEND, CLI imap-msgs add).
//
// What stays here is envelope/header sanity checking that needs the
// SMTP transaction context which pgp_verify does not have:
//   - MIME From must match envelope MAIL FROM (anti-spoofing),
//   - recipient addresses must be well-formed.
func (s *state) CheckBody(ctx context.Context, header textproto.Header, body buffer.Buffer) module.CheckResult {
	if !s.c.requireEncryption {
		return module.CheckResult{}
	}

	// Submission already ran EnforcePolicy at SMTP DATA (one full-body scan).
	if s.msgMeta != nil && s.msgMeta.PGPPolicyVerified {
		return module.CheckResult{}
	}

	s.contentType = header.Get("Content-Type")
	s.mimeFrom = header.Get("From")
	s.secureJoin = header.Get("Secure-Join")

	r, err := body.Open()
	if err != nil {
		return module.CheckResult{
			Reject: true,
			Reason: &exterrors.SMTPError{
				Code:         451,
				EnhancedCode: exterrors.EnhancedCode{4, 0, 0},
				Message:      "Cannot read message body",
				Reason:       "body read error",
				CheckName:    modName,
				Err:          err,
			},
		}
	}
	defer r.Close()

	p := pgp_verify.Policy{
		MailFrom:                   s.mailFrom,
		Recipients:                 s.rcptTos,
		PassthroughSenders:         s.c.passthroughSenders,
		PassthroughRecipients:      s.c.passthroughRecipients,
		AllowSecureJoin:            s.c.allowSecureJoin,
		RequireFromMatchesEnvelope: true,
		ValidateRecipientFormat:    true,
	}

	if err := pgp_verify.EnforcePolicy(header, r, p); err != nil {
		return s.rejectResult(err)
	}
	return module.CheckResult{}
}

// rejectResult stamps the shared SMTPError from pgp_verify with this
// check's name (for log/metric attribution) and adds the envelope
// sender to Misc for debugging.
func (s *state) rejectResult(err error) module.CheckResult {
	var smtpErr *exterrors.SMTPError
	if errors.As(err, &smtpErr) {
		stamped := *smtpErr
		stamped.CheckName = modName
		if stamped.Misc == nil {
			stamped.Misc = map[string]interface{}{}
		}
		stamped.Misc["sender"] = s.mailFrom
		if stamped.Code == 523 {
			s.log.Msg("rejecting unencrypted message",
				"sender", s.mailFrom,
				"content_type", s.contentType,
				"secure_join", s.secureJoin,
			)
		}
		return module.CheckResult{Reject: true, Reason: &stamped}
	}
	return module.CheckResult{
		Reject: true,
		Reason: &exterrors.SMTPError{
			Code:         451,
			EnhancedCode: exterrors.EnhancedCode{4, 0, 0},
			Message:      "Error validating message encryption",
			Reason:       "encryption validation error",
			CheckName:    modName,
			Err:          err,
		},
	}
}

func (s *state) Close() error {
	return nil
}

var (
	_ module.Check      = &Check{}
	_ module.CheckState = &state{}
)

func init() {
	module.Register(modName, New)
}
