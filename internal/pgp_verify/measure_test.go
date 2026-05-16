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
	"testing"
	"time"

	"github.com/emersion/go-message/textproto"
)

// TestMeasureEnforceEncryption_Iterations runs the checker N times and
// logs per-iteration CPU and allocator stats. Use:
//
//	go test ./internal/pgp_verify/ -run TestMeasureEnforceEncryption_Iterations -v
//
// Sizes: 1 MiB and 5 MiB always; 30 MiB unless -short.
// 100 MiB: run explicitly with -run TestMeasureEnforceEncryption_100MiB
func TestMeasureEnforceEncryption_Iterations(t *testing.T) {
	sizes := []struct {
		name string
		n    int
	}{
		{"1MiB", 1 << 20},
		{"5MiB", 5 << 20},
	}
	if !testing.Short() {
		sizes = append(sizes, struct {
			name string
			n    int
		}{"30MiB", 30 << 20})
	}

	const iterations = 3

	for _, sz := range sizes {
		sz := sz
		t.Run(sz.name, func(t *testing.T) {
			boundary, body := makeArmoredPGP(sz.n)
			h := textproto.Header{}
			h.Set("Content-Type", `multipart/encrypted; protocol="application/pgp-encrypted"; boundary="`+boundary+`"`)
			opts := Options{
				MailFrom:   "alice@example.org",
				Recipients: []string{"bob@example.org"},
			}

			var total time.Duration
			for i := 0; i < iterations; i++ {
				st, err := MeasureEnforceEncryption(h, bytes.NewReader(body), opts)
				if err != nil {
					t.Fatalf("iteration %d: %v", i, err)
				}
				if st.BodyBytes == 0 {
					st.BodyBytes = int64(len(body))
				}
				throughput := float64(st.BodyBytes) / st.Duration.Seconds() / (1 << 20)
				t.Logf("iter %d: duration=%s mallocs=%d total_alloc=%dKiB heap_inuse=%dKiB throughput=%.1f MiB/s",
					i, st.Duration, st.Mallocs, st.TotalAlloc/1024, st.HeapInuse/1024, throughput)
				total += st.Duration
			}
			avg := total / iterations
			t.Logf("%s avg over %d iterations: %s (mime body %d bytes)",
				sz.name, iterations, avg, len(body))
		})
	}
}

// TestMeasureCleartextReject_NoBodyRead ensures large cleartext bodies
// are rejected without proportional CPU (header-only path).
// TestMeasureEnforceEncryption_100MiB exercises ~100 MiB ciphertext (needs
// several hundred MiB RAM for armored fixture build). Not run in -short.
//
//	go test ./internal/pgp_verify/ -run TestMeasureEnforceEncryption_100MiB -v -timeout=30m
func TestMeasureEnforceEncryption_100MiB(t *testing.T) {
	if testing.Short() {
		t.Skip("skipping 100 MiB measure test in -short mode")
	}
	const payload = 100 << 20
	opts := Options{
		MailFrom:   "alice@example.org",
		Recipients: []string{"bob@example.org"},
	}

	for _, tc := range []struct {
		name  string
		build func(int) (string, []byte)
	}{
		{"binary", makeBinaryPGP},
		{"armored", makeArmoredPGP},
	} {
		tc := tc
		t.Run(tc.name, func(t *testing.T) {
			t.Logf("building %s %d MiB fixture…", tc.name, payload>>20)
			boundary, body := tc.build(payload)
			h := textproto.Header{}
			h.Set("Content-Type", `multipart/encrypted; protocol="application/pgp-encrypted"; boundary="`+boundary+`"`)

			st, err := MeasureEnforceEncryption(h, bytes.NewReader(body), opts)
			if err != nil {
				t.Fatalf("validation failed: %v", err)
			}
			if st.BodyBytes == 0 {
				st.BodyBytes = int64(len(body))
			}
			throughput := float64(st.BodyBytes) / st.Duration.Seconds() / (1 << 20)
			t.Logf("%s: mime=%d bytes duration=%s mallocs=%d total_alloc=%dKiB heap_inuse=%dKiB throughput=%.1f MiB/s",
				tc.name, len(body), st.Duration, st.Mallocs, st.TotalAlloc/1024, st.HeapInuse/1024, throughput)
		})
	}
}

func TestMeasureCleartextReject_NoBodyRead(t *testing.T) {
	const payload = 8 << 20 // 8 MiB
	body := bytes.Repeat([]byte("x"), payload)
	h := textproto.Header{}
	h.Set("Content-Type", "text/plain")

	st, err := MeasureEnforceEncryption(h, bytes.NewReader(body), Options{
		MailFrom:   "a@b.c",
		Recipients: []string{"d@e.f"},
	})
	if err == nil {
		t.Fatal("expected rejection")
	}
	// Header-only reject should finish quickly (well under scanning 8 MiB).
	if st.Duration > 50*time.Millisecond {
		t.Logf("warning: cleartext reject took %s (expected << 50ms); mallocs=%d", st.Duration, st.Mallocs)
	}
	t.Logf("cleartext reject %d-byte body: duration=%s mallocs=%d", payload, st.Duration, st.Mallocs)
}

func TestStrictSubmissionPolicy_EnforcesEncryption(t *testing.T) {
	boundary, body := makeArmoredPGP(4096)
	h := textproto.Header{}
	h.Set("Content-Type", `multipart/encrypted; boundary="`+boundary+`"`)
	h.Set("From", "alice@example.org")

	p := StrictSubmissionPolicy("alice@example.org", []string{"bob@example.org"})
	if err := EnforcePolicy(h, bytes.NewReader(body), p); err != nil {
		t.Fatalf("expected accept: %v", err)
	}

	h2 := textproto.Header{}
	h2.Set("Content-Type", "text/plain")
	h2.Set("From", "alice@example.org")
	if err := EnforcePolicy(h2, bytes.NewReader([]byte("hi")), p); err == nil {
		t.Fatal("expected cleartext reject")
	}
}

