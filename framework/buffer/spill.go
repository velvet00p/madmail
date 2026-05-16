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

package buffer

import (
	"bytes"
	"fmt"
	"io"
)

// SpillReader buffers all of r. If the stream is at most keepInRAM bytes it
// returns a MemoryBuffer; otherwise it spills the remainder to a file in dir
// (same semantics as SMTP buffer auto mode).
func SpillReader(r io.Reader, dir string, keepInRAM int) (Buffer, error) {
	if keepInRAM <= 0 {
		return BufferInFile(r, dir)
	}

	initial := make([]byte, keepInRAM)
	actualSize, err := io.ReadFull(r, initial)
	if err != nil {
		if err == io.ErrUnexpectedEOF {
			return MemoryBuffer{Slice: initial[:actualSize]}, nil
		}
		if err == io.EOF {
			return MemoryBuffer{}, nil
		}
		return nil, fmt.Errorf("buffer: spill read: %w", err)
	}
	if actualSize < keepInRAM {
		return MemoryBuffer{Slice: initial[:actualSize]}, nil
	}

	return BufferInFile(io.MultiReader(bytes.NewReader(initial), r), dir)
}
