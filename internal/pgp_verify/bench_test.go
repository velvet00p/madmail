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
	"encoding/base64"
	"io"
	"math/rand"
	"mime/multipart"
	"net/textproto"
	"strings"
	"testing"

	msgtextproto "github.com/emersion/go-message/textproto"
)

func makeSEIPDPacket(payloadBytes int, seed int64) []byte {
	buf := make([]byte, 0, payloadBytes+6)
	buf = append(buf, 0xD2, 0xFF,
		byte(payloadBytes>>24), byte(payloadBytes>>16), byte(payloadBytes>>8), byte(payloadBytes))
	payload := make([]byte, payloadBytes)
	rng := rand.New(rand.NewSource(seed))
	_, _ = rng.Read(payload)
	return append(buf, payload...)
}

// writeEncryptedMIME builds RFC 2046-correct multipart/encrypted bodies via
// mime/multipart.Writer so part boundaries are not glued into part 2.
func writeEncryptedMIME(boundary string, writePart2 func(w io.Writer)) []byte {
	var mime bytes.Buffer
	mw := multipart.NewWriter(&mime)
	_ = mw.SetBoundary(boundary)

	p1, _ := mw.CreatePart(textproto.MIMEHeader{
		"Content-Type": {"application/pgp-encrypted"},
	})
	_, _ = p1.Write([]byte("Version: 1"))

	p2, _ := mw.CreatePart(textproto.MIMEHeader{
		"Content-Type": {"application/octet-stream"},
	})
	writePart2(p2)
	_ = mw.Close()
	return mime.Bytes()
}

// makeArmoredPGP builds a syntactically valid multipart/encrypted body
// whose SEIPD packet carries payloadBytes of random data. It does not
// produce a real encrypted message — the walker only cares about
// OpenPGP packet framing, so random bytes are sufficient to exercise
// the decoder + walker + discard loop end-to-end.
func makeArmoredPGP(payloadBytes int) (boundary string, body []byte) {
	boundary = "pgp-test-boundary"
	raw := makeSEIPDPacket(payloadBytes, 1)

	b64 := base64.StdEncoding.EncodeToString(raw)
	var armored strings.Builder
	armored.WriteString("-----BEGIN PGP MESSAGE-----\r\n")
	armored.WriteString("Version: Test\r\n")
	armored.WriteString("\r\n")
	for i := 0; i < len(b64); i += 64 {
		end := i + 64
		if end > len(b64) {
			end = len(b64)
		}
		armored.WriteString(b64[i:end])
		armored.WriteString("\r\n")
	}
	armored.WriteString("=AAAA\r\n")
	armored.WriteString("-----END PGP MESSAGE-----\r\n")

	body = writeEncryptedMIME(boundary, func(w io.Writer) {
		_, _ = io.WriteString(w, armored.String())
	})
	return boundary, body
}

// makeBinaryPGP is like makeArmoredPGP but part 2 is raw binary OpenPGP
// (no ASCII armor) — less CPU in base64 decode, closer to some clients.
func makeBinaryPGP(payloadBytes int) (boundary string, body []byte) {
	boundary = "pgp-test-boundary"
	raw := makeSEIPDPacket(payloadBytes, 2)
	body = writeEncryptedMIME(boundary, func(w io.Writer) {
		_, _ = w.Write(raw)
	})
	return boundary, body
}

func benchEnforceArmored(b *testing.B, payloadBytes int) {
	boundary, body := makeArmoredPGP(payloadBytes)
	h := msgtextproto.Header{}
	h.Set("Content-Type", `multipart/encrypted; protocol="application/pgp-encrypted"; boundary="`+boundary+`"`)
	opts := Options{
		MailFrom:   "alice@example.org",
		Recipients: []string{"bob@example.org"},
	}

	b.SetBytes(int64(len(body)))
	b.ReportAllocs()
	b.ResetTimer()
	for i := 0; i < b.N; i++ {
		if err := EnforceEncryption(h, bytes.NewReader(body), opts); err != nil {
			b.Fatalf("unexpected rejection: %v", err)
		}
	}
}

func BenchmarkEnforceEncryption_Armored1MB(b *testing.B) {
	benchEnforceArmored(b, 1<<20)
}

func BenchmarkEnforceEncryption_Armored5MB(b *testing.B) {
	benchEnforceArmored(b, 5<<20)
}

func BenchmarkEnforceEncryption_Armored30MB(b *testing.B) {
	if testing.Short() {
		b.Skip("skipping 30 MiB bench in -short mode")
	}
	benchEnforceArmored(b, 30<<20)
}

func BenchmarkEnforceEncryption_Armored100MB(b *testing.B) {
	if testing.Short() {
		b.Skip("skipping 100 MiB bench in -short mode")
	}
	benchEnforceArmored(b, 100<<20)
}

func benchEnforceBinary(b *testing.B, payloadBytes int) {
	boundary, body := makeBinaryPGP(payloadBytes)
	h := msgtextproto.Header{}
	h.Set("Content-Type", `multipart/encrypted; boundary="`+boundary+`"`)
	opts := Options{
		MailFrom:   "alice@example.org",
		Recipients: []string{"bob@example.org"},
	}

	b.SetBytes(int64(len(body)))
	b.ReportAllocs()
	b.ResetTimer()
	for i := 0; i < b.N; i++ {
		if err := EnforceEncryption(h, bytes.NewReader(body), opts); err != nil {
			b.Fatalf("unexpected rejection: %v", err)
		}
	}
}

func TestMakeBinaryPGP_Validates(t *testing.T) {
	boundary, body := makeBinaryPGP(1 << 20)
	h := msgtextproto.Header{}
	h.Set("Content-Type", `multipart/encrypted; boundary="`+boundary+`"`)
	if err := EnforceEncryption(h, bytes.NewReader(body), Options{
		MailFrom: "a@b.c", Recipients: []string{"d@e.f"},
	}); err != nil {
		t.Fatal(err)
	}
}

func BenchmarkEnforceEncryption_Binary5MB(b *testing.B) {
	benchEnforceBinary(b, 5<<20)
}

func BenchmarkEnforceEncryption_Binary100MB(b *testing.B) {
	if testing.Short() {
		b.Skip("skipping 100 MiB bench in -short mode")
	}
	benchEnforceBinary(b, 100<<20)
}

// BenchmarkEnforceEncryption_CleartextReject measures the fast reject
// path: Content-Type alone is enough; body is not read.
func BenchmarkEnforceEncryption_CleartextReject(b *testing.B) {
	body := bytes.Repeat([]byte("cleartext content "), 1<<16)
	h := msgtextproto.Header{}
	h.Set("Content-Type", "text/plain")
	h.Set("From", "alice@example.org")

	b.SetBytes(int64(len(body)))
	b.ReportAllocs()
	b.ResetTimer()
	for i := 0; i < b.N; i++ {
		err := EnforceEncryption(h, bytes.NewReader(body), Options{
			MailFrom:   "alice@example.org",
			Recipients: []string{"bob@example.org"},
		})
		if err == nil {
			b.Fatal("expected cleartext to be rejected")
		}
	}
}

// BenchmarkMeasureEnforceEncryption_5MB uses MeasureEnforceEncryption so
// bench output includes the same stats as TestMeasureEnforceEncryption_Iterations.
func BenchmarkMeasureEnforceEncryption_5MB(b *testing.B) {
	boundary, body := makeArmoredPGP(5 << 20)
	h := msgtextproto.Header{}
	h.Set("Content-Type", `multipart/encrypted; boundary="`+boundary+`"`)
	opts := Options{MailFrom: "a@b.c", Recipients: []string{"d@e.f"}}

	b.ReportAllocs()
	for i := 0; i < b.N; i++ {
		st, err := MeasureEnforceEncryption(h, bytes.NewReader(body), opts)
		if err != nil {
			b.Fatal(err)
		}
		b.ReportMetric(float64(st.Duration.Nanoseconds())/1e6, "ms/op")
		b.ReportMetric(float64(st.Mallocs), "mallocs/op")
		b.ReportMetric(float64(st.TotalAlloc)/1024, "KiB_alloc/op")
	}
}
