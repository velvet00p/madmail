package ctl

import (
	"fmt"
	"os"
	"path/filepath"

	frameworkconfig "github.com/themadorg/madmail/framework/config"
	"github.com/themadorg/madmail/internal/confutil"
	maddycli "github.com/themadorg/madmail/internal/cli"
	"github.com/urfave/cli/v2"
)

func init() {
	maddycli.AddSubcommand(&cli.Command{
		Name:  "migrate-pgp-config",
		Usage: "Move submission PGP policy from check.pgp_encryption to endpoint pgp_* directives",
		Description: `One-time migration for Chatmail configs that still use pgp_encryption
inside submission check { }. Rewrites the config file in place (creates a .bak copy).

After migration, PGP is enforced once at SMTP DATA instead of twice.`,
		Flags: []cli.Flag{
			&cli.StringFlag{
				Name:  "config",
				Usage: "Path to maddy.conf (default: from MADDY_CONFIG or /etc/<binary>/<binary>.conf)",
			},
			&cli.BoolFlag{
				Name:  "dry-run",
				Usage: "Print changes without writing the file",
			},
		},
		Action: runMigratePGPConfig,
	})
}

func runMigratePGPConfig(c *cli.Context) error {
	path := c.String("config")
	if path == "" {
		path = frameworkconfig.ConfigFile()
	}
	if path == "" {
		return fmt.Errorf("config path not set; use --config")
	}

	data, err := os.ReadFile(path)
	if err != nil {
		return err
	}

	newContent, changed, notes := confutil.MigrateSubmissionPGP(string(data))
	if !changed {
		fmt.Printf("No migration needed for %s\n", path)
		return nil
	}

	for _, n := range notes {
		fmt.Println(" •", n)
	}

	if c.Bool("dry-run") {
		fmt.Printf("\n(dry-run) would update %s\n", path)
		return nil
	}

	backup := path + ".bak"
	if err := os.WriteFile(backup, data, 0644); err != nil {
		return fmt.Errorf("backup %s: %w", backup, err)
	}
	if err := os.WriteFile(path, []byte(newContent), 0644); err != nil {
		return err
	}
	fmt.Printf("✅ Updated %s (backup at %s)\n", path, filepath.Base(backup))
	fmt.Println("Run: madmail reload   # or systemctl restart maddy")
	return nil
}
