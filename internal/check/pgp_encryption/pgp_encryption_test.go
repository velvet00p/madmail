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
	"strings"
	"testing"
	"time"

	"github.com/emersion/go-message/textproto"
	"github.com/themadorg/madmail/framework/buffer"
	"github.com/themadorg/madmail/framework/module"
)

func TestCheckBody_SkipsWhenPGPPolicyVerified(t *testing.T) {
	c := &Check{requireEncryption: true}
	st, err := c.CheckStateForMsg(context.Background(), &module.MsgMetadata{
		ID:                "test-skip",
		PGPPolicyVerified: true,
	})
	if err != nil {
		t.Fatal(err)
	}
	s := st.(*state)
	s.mailFrom = "alice@example.org"
	s.rcptTos = []string{"bob@example.org"}

	h := textproto.Header{}
	h.Set("Content-Type", "text/plain")
	body := buffer.MemoryBuffer{Slice: []byte(strings.Repeat("cleartext ", 1<<18))}

	start := time.Now()
	res := s.CheckBody(context.Background(), h, body)
	if res.Reject {
		t.Fatalf("expected skip when PGPPolicyVerified, got reject: %v", res.Reason)
	}
	if time.Since(start) > 5*time.Millisecond {
		t.Fatalf("CheckBody took %s; expected fast skip without body scan", time.Since(start))
	}
}

func TestCheckBody_RejectsCleartextWithoutFlag(t *testing.T) {
	c := &Check{requireEncryption: true}
	st, err := c.CheckStateForMsg(context.Background(), &module.MsgMetadata{ID: "test-reject"})
	if err != nil {
		t.Fatal(err)
	}
	s := st.(*state)
	s.mailFrom = "alice@example.org"
	s.rcptTos = []string{"bob@example.org"}

	h := textproto.Header{}
	h.Set("Content-Type", "text/plain")
	h.Set("From", "alice@example.org")
	body := buffer.MemoryBuffer{Slice: []byte("not encrypted")}

	res := s.CheckBody(context.Background(), h, body)
	if !res.Reject {
		t.Fatal("expected cleartext to be rejected")
	}
}
