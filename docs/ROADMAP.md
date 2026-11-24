# Roadmap

1. Make the project documentation and development entry points easier to use. Verify each documented command and link.
2. Measure behavior against the original game with isolated saves. Document the evidence for the interpreter shortcuts and investigate unstable comparisons.
3. Separate the platform interface, interpreter, save format and presentation logic. The documented interface, reference port and port template exist; moving the desktop, web and Vita hosts onto the interface, and extracting the full instruction table and value model from `clr`, remain. The cold trial of the porting guide with a fresh agent is pending.
4. Repeat performance measurements with validated counters on real hardware and document the exact commands and conditions.
5. Review the remaining discrepancies and agree on the next development milestone.

Current capabilities and unresolved issues are summarized in [status](STATUS.md). See [testing](TESTING.md) for the verification commands.
