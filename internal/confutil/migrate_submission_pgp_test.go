package confutil

import (
	"strings"
	"testing"
)

const oldSubmissionSnippet = `
submission tls://0.0.0.0:465 tcp://0.0.0.0:587 {
    auth &local_authdb
    source $(local_domains) {
        check {
            require_tls
            pgp_encryption {
                require_encryption yes
                allow_secure_join yes
                passthrough_senders relay@example.org
            }
        }
        destination postmaster $(local_domains) {
            deliver_to &local_routing
        }
    }
}
`

func TestMigrateSubmissionPGP(t *testing.T) {
	out, changed, notes := MigrateSubmissionPGP(oldSubmissionSnippet)
	if !changed {
		t.Fatalf("expected change, notes=%v", notes)
	}
	if strings.Contains(out, "pgp_encryption {") {
		t.Error("pgp_encryption block should be removed")
	}
	if !strings.Contains(out, "pgp_allow_secure_join yes") {
		t.Error("missing pgp_allow_secure_join")
	}
	if !strings.Contains(out, "pgp_passthrough_senders relay@example.org") {
		t.Error("missing passthrough senders")
	}

	out2, changed2, _ := MigrateSubmissionPGP(out)
	if changed2 {
		t.Error("second migration should be idempotent")
	}
	if out != out2 {
		t.Error("content changed on second pass")
	}
}
