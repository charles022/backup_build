Below is a **clean, end-to-end engineering document** you can hand directly to your team.
It is written as an **implementation and operations guide**, not as a conversational recap.
The conversation history can be kept as supplemental context, but this document is **self-sufficient**.

---

# Dev Backup, Restore, and Synchronization System

**Engineering Implementation & Operations Guide**

---

## 1. Executive Summary

This document specifies a **Btrfs-based backup, restore, and workstation/server synchronization system** designed to:

* Preserve **plain, unencrypted data locally**
* Store **encrypted + compressed backups in the cloud**
* Support **incremental snapshots with periodic full anchors**
* Allow **wipe-and-reinstall recovery** of either machine
* Keep **local server (LS)** and **workstation (WS)** usable concurrently
* Maintain **cloud storage as cold redundancy**, not a daily dependency
* Avoid AWS and use **Cloudflare R2 Object Storage**

The system is **deterministic, testable, and auditable**, with clear separation between:

* snapshot creation
* artifact generation
* artifact storage
* cloud replication
* restore and rebuild operations

---

## 2. Core Design Principles

### 2.1 Non-Negotiable Goals

1. **Local performance**

   * `/home/chuck/code` is always plaintext and directly usable
   * No encryption or compression at rest locally

2. **Cloud security**

   * All cloud data is encrypted client-side
   * Cloud provider never sees plaintext

3. **Restore guarantees**

   * Any snapshot can be restored without replaying full history
   * Latest snapshot restore does not require downloading all data

4. **Minimal coupling**

   * Cloud is storage only
   * No VM required in the cloud
   * Restore logic lives on LS/WS

---

## 3. System Roles and Responsibilities

### 3.1 Machines

#### Workstation (WS)

* Primary development machine
* Path: `/home/chuck/code`
* Creates snapshots
* Generates encrypted backup artifacts
* Initiates monthly backups
* Can request restores from LS

#### Local Server (LS)

* Canonical backup repository
* Holds:

  * encrypted backup artifacts
  * snapshot manifests
  * age private key
* Pushes backups to Cloudflare R2
* Acts as restore engine
* Provides a writable working copy of `/home/chuck/code`

#### Cloudflare R2

* Cold, encrypted object storage
* Holds:

  * encrypted snapshot artifacts
  * manifests
* Never performs restore logic

---

## 4. Data Model

### 4.1 Dataset

* **Dataset name:** `dev`
* **Subvolume path:** `/home/chuck/code`
* **Filesystem:** Btrfs (required)

### 4.2 Snapshot Naming

```
dev@YYYY-MM
```

Examples:

* `dev@2026-01`
* `dev@2026-02`

### 4.3 Artifact Types

| Type        | Description                         |
| ----------- | ----------------------------------- |
| Anchor      | Full, self-contained snapshot       |
| Incremental | Delta relative to previous snapshot |

Artifact filenames:

```
dev@YYYY-MM.full.send.zst.age
dev@YYYY-MM.incr.from_YYYY-MM.send.zst.age
```

---

## 5. Backup Strategy (Anchor + Incremental)

### 5.1 Snapshot Schedule

* Monthly snapshots
* Monthly backup execution

### 5.2 Anchor Creation Rules

A **new full anchor** is created if **either**:

1. Sum of incremental artifact sizes since last anchor ≥ size of last anchor artifact
2. 12 months have passed since last anchor

All anchors are retained indefinitely.

---

## 6. Manifest and Metadata

### 6.1 Manifest v2 (Authoritative)

File:

```
/srv/btrfs-backups/dev/manifests/snapshots_v2.tsv
```

Columns:

```
ts | label | type | parent | bytes | sha256 | local_path | object_key
```

This manifest is the **single source of truth** for:

* restore planning
* cloud pulls
* integrity verification

---

## 7. Encryption and Compression

* **Compression:** `zstd`
* **Encryption:** `age`
* **Key ownership:**

  * LS holds **private key**
  * WS holds **public key only**
* Decryption occurs:

  * on LS for restores
  * streamed to WS when requested

---

## 8. Directory Layout (LS)

```
/srv/btrfs-backups/dev/
├── artifacts/
│   ├── anchors/
│   └── incr/
├── manifests/
│   ├── snapshots_v2.tsv
│   └── state.env
├── keys/
│   ├── ls_dev_backup.key
│   └── ls_dev_backup.pub
├── restore/
│   └── worktree_receive/
├── tmp/
├── logs/
└── locks/
```

---

## 9. Script Inventory (Authoritative)

### Initialization

| Script       | Purpose                |
| ------------ | ---------------------- |
| `init_LS.sh` | Build LS from clean OS |
| `init_WS.sh` | Build WS from clean OS |

### Backup & Artifact Flow

| Script                        | Purpose                                |
| ----------------------------- | -------------------------------------- |
| `ws_dev_snapshot.sh`          | Create monthly snapshot                |
| `ws_dev_export_and_push.sh`   | Create artifact and send to LS         |
| `ws_dev_run_month.sh`         | Monthly harness                        |
| `ls_dev_register_artifact.sh` | Register artifact + policy enforcement |
| `push_to_cloud.sh`            | Push artifacts/manifests to R2         |

### Restore & Recovery

| Script                 | Purpose                             |
| ---------------------- | ----------------------------------- |
| `pull_from_cloud.sh`   | Pull artifacts/manifests from R2    |
| `store_on_LS.sh`       | Place pulled artifacts into LS repo |
| `restore_backup_LS.sh` | Restore LS working tree             |
| `LS_send.sh`           | Stream restore data to WS           |
| `LS_request.sh`        | WS restore request                  |

### Maintenance

| Script                      | Purpose                       |
| --------------------------- | ----------------------------- |
| `migrate_manifest_to_v2.sh` | Upgrade legacy manifests      |
| `ls_dev_plan_restore.sh`    | Compute minimal restore chain |
| `ls_dev_latest_label.sh`    | Query latest snapshot         |

---

## 10. Initialization Procedures

### 10.1 LS Initialization

```bash
sudo ./init_LS.sh
```

Effects:

* Installs dependencies
* Creates repo structure
* Generates age keypair
* Initializes manifests and state
* Installs all LS scripts

### 10.2 WS Initialization

```bash
./init_WS.sh
```

Effects:

* Ensures `/home/chuck/code` is a Btrfs subvolume
* Creates snapshot root
* Installs WS scripts
* Sets up configuration files

---

## 11. Monthly Backup Procedure

On WS:

```bash
ws_dev_run_month.sh YYYY-MM
```

On LS:

```bash
push_to_cloud.sh
```

Result:

* Snapshot created
* Artifact encrypted + compressed
* Artifact registered
* LS working tree updated
* Cloud mirror updated

---

## 12. Restore Scenarios

### 12.1 Restore LS After Wipe

```bash
pull_from_cloud.sh latest
store_on_LS.sh /tmp/dev-backup-cloud-pull
restore_backup_LS.sh latest
```

### 12.2 Restore WS From LS

```bash
LS_request.sh latest
```

---

## 13. LS ↔ WS Shared Working Copy

* LS automatically updates `/home/chuck/code` after each snapshot
* Working copy is derived from restored snapshots
* Edits on LS are allowed but **not automatically synced**
* Manual LS→WS sync can be done with `rsync` when needed

---

## 14. Cloudflare R2 Configuration Notes

* S3-compatible
* Endpoint format:

```
https://<ACCOUNT_ID>.r2.cloudflarestorage.com
```

* Recommended: `rclone >= 1.59`
* No egress fees (storage + operations apply)

---

## 15. Safety Guarantees

* Backup artifacts are immutable
* Working copies are replaceable
* Old states are never destroyed automatically
* All destructive operations require explicit action

---

## 16. Operational Philosophy

This system intentionally:

* Prefers **clarity over cleverness**
* Avoids “magic” tools
* Allows inspection at every stage
* Keeps cloud as a dumb storage backend
* Scales from “single dev directory” to multi-TB datasets

---

## 17. Future Enhancements (Out of Scope)

* Bidirectional snapshot sync
* Automatic conflict detection
* Multi-user workflows
* Snapshot pruning policies
* GUI monitoring

---

## 18. Final Notes to Engineering Team

* Implement scripts **exactly as specified**
* Do not optimize prematurely
* Validate each stage independently
* Treat manifests as authoritative
* Keep encryption boundaries intact

This design was intentionally built to be **boring, correct, and recoverable**.

