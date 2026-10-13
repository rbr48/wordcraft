# wordcraft-print

Submit a print-ready PDF to the system spooler, for WordCraft's File › Print. The engine
generates the PDF and decides what to print; this crate discovers printers and submits jobs.
It has no dependency on the engine or on a UI framework.

Ported from the `craft-print` crate of storytold/craft-libs (MIT OR Apache-2.0), at
`d27646a5f7c8b4fc85d9dd3213b8c88b58e0088d`, and kept under the workspace licence.

```rust,no_run
let job = wordcraft_print::Job { title: "Report".into(), ..Default::default() };
let receipt = wordcraft_print::submit(&pdf_bytes, &job)?;
```

The caller generates the PDF and checks document permissions. This crate discovers printers,
exposes CUPS driver options, opens Windows printer preferences and submits jobs with copies,
collation, duplex and colour. `command_preview` returns diagnostics without creating files or
starting a process.

- **macOS/Linux:** CUPS `lpstat`, `lpoptions` and `lp`. Jobs stream through stdin. Exact-size
  sheets use `fit-to-page=false`; shrink-to-fit uses IPP `print-scaling=auto-fit`.
- **Windows:** inbox Windows PowerShell, `Windows.Data.Pdf` and `System.Drawing.Printing`. Each
  sheet is rasterized, with paper and orientation selected per page. Exact-size mode preserves
  physical size and clips hardware margins. Shrink-to-fit uses matching driver paper or its
  default, centers in the printable area and never enlarges smaller sheets. Long-edge duplex
  uses `Vertical`; short-edge uses `Horizontal`. Driver-specific defaults remain in the Windows
  printer preferences.
- **Browser and other targets:** discovery is empty and submission reports an unsupported
  platform. The caller can offer PDF export instead.

Windows jobs use a random, exclusively created temporary file, removed on drop on success and
failure. CUPS jobs never use a PDF temporary file. Printing is synchronous, so UI callers should
submit on a worker thread. Receipts are backend messages, not portable job IDs.

`Job::duplex = None` keeps the queue's duplex default; `Some(Duplex::Off)` forces simplex.
`Job::scaling` defaults to `Scaling::Exact`. Windows raster allocation is bounded to 64 million
pixels per sheet and 32768 per dimension; sheets up to 128 inches are supported and invalid sizes
are reported.

`cargo test -p wordcraft-print` tests the CUPS transport (including pipe-sized jobs and refusal),
printer parsing, PPD options and argument mapping. On Windows it also parses the embedded
PowerShell script, checks sheet geometry, duplex and raster bounds without hardware, and renders a
mixed-orientation PDF through WinRT. The queue test needs a paused test printer and skips where
the driver or administrative rights are missing (`WORDCRAFT_PRINT_REQUIRE_QUEUE=1` makes it
mandatory). Physical printer fidelity still needs hardware verification.
