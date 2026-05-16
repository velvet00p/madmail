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
	"io"
	"os"
	"path/filepath"
	"testing"
)

func TestSpillReader_SmallStaysInRAM(t *testing.T) {
	b, err := SpillReader(bytes.NewReader([]byte("hi")), t.TempDir(), 1024)
	if err != nil {
		t.Fatal(err)
	}
	defer b.Remove()
	if _, ok := b.(MemoryBuffer); !ok {
		t.Fatalf("expected MemoryBuffer, got %T", b)
	}
}

func TestSpillReader_LargeSpillsToFile(t *testing.T) {
	payload := bytes.Repeat([]byte("x"), 2048)
	b, err := SpillReader(bytes.NewReader(payload), t.TempDir(), 512)
	if err != nil {
		t.Fatal(err)
	}
	defer b.Remove()
	fb, ok := b.(FileBuffer)
	if !ok {
		t.Fatalf("expected FileBuffer, got %T", b)
	}
	r, err := fb.Open()
	if err != nil {
		t.Fatal(err)
	}
	got, err := io.ReadAll(r)
	_ = r.Close()
	if err != nil {
		t.Fatal(err)
	}
	if !bytes.Equal(got, payload) {
		t.Fatalf("len %d want %d", len(got), len(payload))
	}
}

func TestFileBuffer_LinkAt(t *testing.T) {
	dir := t.TempDir()
	srcPath := filepath.Join(dir, "src")
	if err := os.WriteFile(srcPath, []byte("queue-body"), 0o600); err != nil {
		t.Fatal(err)
	}
	destPath := filepath.Join(dir, "dest")
	if err := (FileBuffer{Path: srcPath, LenHint: 10}).LinkAt(destPath); err != nil {
		t.Fatal(err)
	}
	if _, err := os.Stat(srcPath); err != nil {
		t.Fatal("source link should remain")
	}
	got, err := os.ReadFile(destPath)
	if err != nil {
		t.Fatal(err)
	}
	if string(got) != "queue-body" {
		t.Fatalf("got %q", got)
	}
}
