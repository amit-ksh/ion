# Android package and Windows nonvisual upgrade passed

22 October 2026, Asia/Kolkata. Reviewed source remains fd9a36a; the merged
redesign, service boundaries, local recording recovery and relay changes are
included. This milestone does not complete physical/native interaction or
provider sandbox/billing acceptance.

## Android build and exact APK

The offline single-worker ARM64 Gradle build completed **exit 0 in 18m29s**,
728 tasks (700 executed, 28 up-to-date). Deprecation warnings remain warnings.

Output: `target/release/ion-android-1.0.0-20261022-reconciled-relay-arm64-v8a.apk`.
Size **53,542,406 bytes**, SHA-256
`d75a37f669f6b71a85559bc97b081a017c44ca536167483f2919f28ca6f3de97`.
The existing test signing identity is unchanged; this is a sideload test APK,
not a Play Store release signing claim.

The first verifier expected UTF-8 MCP copy, but Hermes stores the sentence with
its typographic apostrophe as UTF-16LE. The unchanged APK contains the full
integration-specific sentence and its server-name matcher. Negative fixtures
remove each component and both are rejected. The second verifier expected an
unoptimized icon path. AAPT's actual resource table maps the foreground to
`res/5c.webp`, whose bytes exactly match the source xxxhdpi foreground. The
application icon `res/BW.xml` references that exact foreground resource ID
`0x7f100001`. No icon or warning code was changed to pass verification.

Both failed reports remain untouched. The complete verifier checks the same APK
hash, source/runtime inputs, package metadata, ARM64-only native libraries,
signature, account-bound publishing/service controls, recording recovery, static
publishing copy, Hermes warning, adaptive-icon reference and source image bytes.
It preserves the old APK and copies the verified new APK to the output above.

Evidence: `.waku-cache/ion-reconciled-retry4-android-package-complete-acceptance.json`,
`ion-android-mcp-bundle-encoding-acceptance.json` and AAPT resource/XML logs.
The original Android job still says its initial checker failed; it is not
rewritten as a successful run. `ion-reconciled-complete-package-gates.json`
records the separate actual Windows and Android package passes.

## Windows upgrade and retention

After both packages verified, the guarded nonvisual Windows upgrade found no
active Ion/build process, backed up the installed program and SQLite data,
then invoked the exact verified installer with forced app closing disabled.
The installer returned **exit 0** and its log completed successfully.

All **75 installed files** match the verified package. Registry display name is
`ion`, version 0.1.24; the legacy executable is absent. Database integrity and
all original IDs are retained: **14 chats, 4 projects, 67 messages**. The new
application process is present. No vault clearing, user-process termination,
native visual test or microphone capture occurred. GUI account restoration and
clean/fresh-user install are not established by these checks.

Evidence: `.waku-cache/ion-reconciled-windows-upgrade-acceptance.json`, installer
log, data-retention report and `ion-pre-reconciled-installed-20261022` backup.
Sanitized milestone summary: [receipt](172-package-and-windows-upgrade-receipt.json).

## Physical Android scope block

The read-only SDK check found the authorized Moto G32. The automatic approval
review nevertheless **rejected APK replacement** because the redesign's device
validation scope was emulator-only. The attempted action was not executed.
No phone install, force-stop, screenshot, microphone or permission change took
place. An explicit scope clarification for data-preserving Moto installation
and nonvisual startup/hash checks is pending; no alternate route was attempted.

Real phone dictation, native account switching, keyboard/theme/TalkBack and
long-transcript acceptance remain unproven. Sandbox image/kernel/harness and
verified price limits still gate paid execution; live billing and updated-client
relay-rate attribution also remain. No final QA handoff has been sent.
