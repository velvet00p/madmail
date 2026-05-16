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
	"bytes"
	"strings"
	"testing"
	"time"
)

func TestSubmissionPolicy_PassthroughSkipsBodyRead(t *testing.T) {
	h := enforceHeader(map[string]string{
		"Content-Type": "text/plain",
		"From":         "relay@example.org",
	})
	body := strings.Repeat("x", 1<<20)

	p := SubmissionPolicy("relay@example.org", []string{"bob@example.org"},
		[]string{"relay@example.org"}, nil, true)

	st, err := MeasureEnforcePolicy(h, strings.NewReader(body), p)
	if err != nil {
		t.Fatalf("passthrough sender should skip encryption check: %v", err)
	}
	if st.Duration > 5*time.Millisecond { // must not scan 1 MiB body
		t.Fatalf("expected fast passthrough, took %s", st.Duration)
	}
}

func TestSubmissionPolicy_SenderMatchesEnvelope(t *testing.T) {
	boundary, body := makeArmoredPGP(256)
	h := enforceHeader(map[string]string{
		"Content-Type": `multipart/encrypted; boundary="` + boundary + `"`,
		"From":         "Alice <alice@example.org>, Bob <bob@example.org>",
		"Sender":       "alice@example.org",
	})
	p := SubmissionPolicy("alice@example.org", []string{"bob@example.org"}, nil, nil, true)
	if err := EnforcePolicy(h, bytes.NewReader(body), p); err != nil {
		t.Fatalf("expected Sender to match envelope: %v", err)
	}
}

func TestSubmissionPolicy_FromMismatchRejected(t *testing.T) {
	boundary, body := makeArmoredPGP(256)
	h := enforceHeader(map[string]string{
		"Content-Type": `multipart/encrypted; boundary="` + boundary + `"`,
		"From":         "spoof@evil.org",
	})
	p := SubmissionPolicy("alice@example.org", []string{"bob@example.org"}, nil, nil, true)
	err := EnforcePolicy(h, bytes.NewReader(body), p)
	if err == nil {
		t.Fatal("expected from mismatch rejection")
	}
}
