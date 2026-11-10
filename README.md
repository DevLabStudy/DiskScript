# DiskScript (dsk)

**DiskScript** is a lightweight, human-readable Domain-Specific Language (DSL) designed to automate and simplify disk management operations—such as partitioning, wiping, and formatting storage media (NTFS, exFAT, FAT32). 

Tired of remembering archaic `diskpart` syntax, flags, and sequence flows? DiskScript provides a clean, declarative abstraction layer that compiles down to robust system commands.

---

## Features

- **Declarative Syntax:** Define what you want to achieve with your storage, not the tedious step-by-step CLI commands.
- **Multi-Format Support:** Easily target NTFS, exFAT, or FAT32 with customized volume labels.
- **Safety First:** Built-in safeguards, confirmation prompts, and explicit drive targeting to prevent accidental data loss.
- **Cross-Platform Foundation:** Built to run smoothly and integrate with system disk utilities.

---

## Example Usage

Here is what a typical DiskScript file looks like for formatting a target external drive:

```diskscript
disk "Kingston_USB" {
    select target = largest_removable
    action = wipe_and_partition
    
    format {
        filesystem = exFAT
        label = "BACKUP_2026"
        quick = true
    }
}
