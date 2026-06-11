# Manual pages

`madmail.1.scd` is the maintained source (scdoc format, groff **man** macros).

```bash
make man          # regenerate docs/man/madmail.1 (requires scdoc)
make man-lint     # groff render smoke test
make man-check    # man + lint when tools are installed
```

The rendered `madmail.1` is committed so builds succeed without scdoc; `crates/chatmail/src/ctl/docs.rs`
embeds it at compile time. Run `make man` to refresh from `madmail.1.scd` (`@VERSION@` from `CARGO_PKG_VERSION`).

Style reference: [man-pages(7)](https://man7.org/linux/man-pages/man7/man-pages.7.html),
[groff_man(7)](https://man7.org/linux/man-pages/man7/groff_man.7.html),
[groff_man_style(7)](https://man7.org/linux/man-pages/man7/groff_man_style.7.html).