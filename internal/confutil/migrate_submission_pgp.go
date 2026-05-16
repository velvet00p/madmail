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

// Package confutil holds small helpers for editing maddy.conf text.
package confutil

import (
	"fmt"
	"strings"
)

// MigrateSubmissionPGP moves PGP policy from check.pgp_encryption inside
// submission's source.check to submission-level pgp_* directives (one SMTP
// DATA scan). Idempotent when already migrated.
func MigrateSubmissionPGP(content string) (string, bool, []string) {
	if strings.Contains(content, "pgp_allow_secure_join") &&
		!strings.Contains(content, "pgp_encryption") {
		return content, false, nil
	}

	lines := strings.Split(content, "\n")
	var out []string
	var notes []string
	var extracted []string
	changed := false

	inSubmission := false
	depth := 0

	for i := 0; i < len(lines); i++ {
		line := lines[i]
		trim := strings.TrimSpace(line)

		if !inSubmission {
			if strings.HasPrefix(trim, "submission ") && strings.Contains(trim, "{") {
				inSubmission = true
				depth = braceDelta(trim)
			}
			out = append(out, line)
			continue
		}

		// Inside submission block.
		if strings.HasPrefix(trim, "pgp_encryption") {
			blockLines, endIdx := collectBraceBlock(lines, i)
			extracted = append(extracted, pgpEncryptionBlockToDirectives(blockLines)...)
			i = endIdx
			changed = true
			notes = append(notes, "removed check.pgp_encryption from submission (duplicate body scan)")
			depth += braceDelta(strings.Join(blockLines, "\n"))
			continue
		}

		out = append(out, line)
		depth += braceDelta(trim)
		if depth <= 0 {
			inSubmission = false
			depth = 0
		}
	}

	if !changed {
		return content, false, nil
	}

	if len(extracted) > 0 && !strings.Contains(strings.Join(out, "\n"), "pgp_allow_secure_join") {
		out = insertBeforeSubmissionSource(out, extracted)
		notes = append(notes, fmt.Sprintf("added %d submission-level pgp_* directive(s)", len(extracted)))
	}

	return strings.Join(out, "\n"), true, notes
}

func braceDelta(s string) int {
	n := 0
	for _, c := range s {
		switch c {
		case '{':
			n++
		case '}':
			n--
		}
	}
	return n
}

// collectBraceBlock returns inner lines of a { ... } block starting at startIdx
// (the line with opening {) and the index of the closing } line.
func collectBraceBlock(lines []string, startIdx int) ([]string, int) {
	depth := braceDelta(lines[startIdx])
	if depth <= 0 {
		return nil, startIdx
	}
	var inner []string
	i := startIdx + 1
	for i < len(lines) && depth > 0 {
		inner = append(inner, lines[i])
		depth += braceDelta(lines[i])
		i++
	}
	return inner, i - 1
}

func pgpEncryptionBlockToDirectives(innerLines []string) []string {
	allowSecureJoin := "yes"
	var passthroughSenders, passthroughRecipients string

	for _, line := range innerLines {
		trim := strings.TrimSpace(line)
		switch {
		case strings.HasPrefix(trim, "allow_secure_join"):
			if f := strings.Fields(trim); len(f) >= 2 {
				allowSecureJoin = f[1]
			}
		case strings.HasPrefix(trim, "passthrough_senders"):
			passthroughSenders = strings.TrimSpace(strings.TrimPrefix(trim, "passthrough_senders"))
		case strings.HasPrefix(trim, "passthrough_recipients"):
			passthroughRecipients = strings.TrimSpace(strings.TrimPrefix(trim, "passthrough_recipients"))
		}
	}

	var dirs []string
	dirs = append(dirs, fmt.Sprintf("    pgp_allow_secure_join %s", allowSecureJoin))
	if passthroughSenders != "" {
		dirs = append(dirs, "    pgp_passthrough_senders "+passthroughSenders)
	}
	if passthroughRecipients != "" {
		dirs = append(dirs, "    pgp_passthrough_recipients "+passthroughRecipients)
	}
	return dirs
}

func insertBeforeSubmissionSource(lines, directives []string) []string {
	inSubmission := false
	depth := 0
	for i, line := range lines {
		trim := strings.TrimSpace(line)
		if !inSubmission {
			if strings.HasPrefix(trim, "submission ") && strings.Contains(trim, "{") {
				inSubmission = true
				depth = braceDelta(trim)
			}
			continue
		}
		depth += braceDelta(trim)
		if strings.HasPrefix(trim, "source ") && depth == 1 {
			var result []string
			result = append(result, lines[:i]...)
			result = append(result, "    # PGP-only: one scan at SMTP DATA (migrated from check.pgp_encryption)")
			result = append(result, directives...)
			result = append(result, lines[i:]...)
			return result
		}
		if depth <= 0 {
			break
		}
	}
	return append(lines, directives...)
}
