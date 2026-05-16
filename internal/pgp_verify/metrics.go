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
	"runtime"
	"time"

	"github.com/emersion/go-message/textproto"
)

// RunStats captures CPU time and allocator deltas for one policy run.
// Use MeasureEnforceEncryption / MeasureEnforcePolicy in tests and
// benchmarks to compare iterations without external profilers.
type RunStats struct {
	BodyBytes int64

	Duration time.Duration

	// Mallocs and TotalAlloc are deltas across the measured call (after GC).
	Mallocs    uint64
	TotalAlloc uint64
	HeapInuse  uint64
}

// MeasureEnforceEncryption runs EnforceEncryption once and returns timing
// and memory deltas. body is read until EOF; BodyBytes is len(body) when
// body implements interface{ Len() int } or after read from bytes.Reader.
func MeasureEnforceEncryption(header textproto.Header, body io.Reader, opts Options) (RunStats, error) {
	return measureRun(func() error {
		return EnforceEncryption(header, body, opts)
	}, bodySizeHint(body))
}

// MeasureEnforcePolicy runs EnforcePolicy once with the same metrics.
func MeasureEnforcePolicy(header textproto.Header, body io.Reader, p Policy) (RunStats, error) {
	return measureRun(func() error {
		return EnforcePolicy(header, body, p)
	}, bodySizeHint(body))
}

func bodySizeHint(body io.Reader) int64 {
	type sizer interface{ Len() int }
	if s, ok := body.(sizer); ok {
		return int64(s.Len())
	}
	return 0
}

func measureRun(fn func() error, bodyBytes int64) (RunStats, error) {
	runtime.GC()
	var before runtime.MemStats
	runtime.ReadMemStats(&before)

	start := time.Now()
	err := fn()
	elapsed := time.Since(start)

	var after runtime.MemStats
	runtime.ReadMemStats(&after)

	st := RunStats{
		BodyBytes:  bodyBytes,
		Duration:   elapsed,
		Mallocs:    after.Mallocs - before.Mallocs,
		TotalAlloc: after.TotalAlloc - before.TotalAlloc,
		HeapInuse:  after.HeapInuse,
	}
	return st, err
}
