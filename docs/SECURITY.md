# BonsCompte Security Model

## Overview

BonsCompte implements a **two-layer approval system** for account recovery without relying on email/SMTP. This document explains the security model, approval workflows, and trust assumptions.

---

## Core Principles

1. **No Email Dependency**: Password recovery works without SMTP, reducing infrastructure requirements and email-based attack vectors
2. **Human Approval**: Account security relies on trusted project members verifying sensitive operations
3. **Token Versioning**: All sessions are invalidated when security-sensitive changes occur
4. **Project-Based Trust**: Security is scoped to project membership relationships
5. **Multi-Signature Approval**: Regular members require a quorum (33%) while admins can approve individually

---

## Two-Layer Approval System

### Layer 1: Global (System Admin)

**Who**: Users with CLI access (`bonscompte-admin` tool)

**Powers**:
- Full account restoration (sets `user.state = Active`)
- Activates ALL project memberships
- Emergency override for any approval

**Use Cases**:
- Password resets initiated by admin
- Account recovery when user has no accessible projects
- Solo projects (where user is the only member)

**CLI Commands**:
```bash
# Reset password (triggers approval workflow)
bonscompte-admin reset-password <username>

# Approve user globally (full restoration)
bonscompte-admin approve <username>

# Revoke user access entirely
bonscompte-admin revoke <username>
```

### Layer 2: Project-Specific (Member Voting)

**Who**: Active members of projects where the user participates

**Voting Thresholds**:
| Member Count | Project Admin Vote | Regular Member Votes Required |
|--------------|-------------------|------------------------------|
| 1 user       | N/A (only sysadmin can approve) | N/A |
| 2 users      | 1 (instant approval) | 1 (50%) |
| 3 users      | 1 (instant approval) | 1 (33%) |
| 4+ users     | 1 (instant approval) | ceil(33% × count) |

**Approval Formula**:
```
required_votes = ceil(member_count × 0.33)
```

**Effects**:
- Project admin approval → Activates membership in THAT project only
- Quorum approval → Activates membership in THAT project only
- Rejection → Permanent block (requires CLI admin intervention)

---

## Security Events Triggering Approval

### Current (Implemented)

1. **Password Change** (authenticated user)
   - User changes password via `/users/me/password`
   - Token version increments → all existing sessions invalidated
   - User state → `PendingApproval`
   - All project memberships → `pending`
   - Approval required to regain access

2. **Password Reset** (CLI admin)
   - Admin runs `reset-password` command
   - Temporary password generated
   - Token version increments → all sessions invalidated
   - User state → `PendingApproval`
   - User must login with temp password and await approval

### Future (Planned)

3. **New Device Login** (Phase 2)
   - Login from unrecognized device/IP
   - Requires approval from existing trusted devices

4. **Role Elevation** (Phase 2)
   - User promoted to admin/editor in a project
   - Current admin approves OR quorum of members approve

---

## Token Versioning & Session Invalidation

Every user has a `token_version` field embedded in their JWT tokens.

**Invalidation Triggers**:
- Password change (user-initiated)
- Password reset (admin-initiated)
- Account revocation
- Manual token bump (future feature)

**How It Works**:
1. JWT includes `token_version` claim (e.g., version 5)
2. On password change, database updates `token_version = 6`
3. Next API request with old token (version 5) is rejected
4. User must re-authenticate to get new token with version 6

**Guarantees**:
- Old tokens become invalid **immediately** (no grace period)
- No distributed cache needed (version checked on every request)
- Survives server restarts (version persisted in database)

---

## Quarantine Mode (PendingApproval State)

When a user is in `PendingApproval` state:

**Allowed Operations**:
- `GET /users/me` - View own profile
- `PUT /users/me/profile` - Update display name
- `GET /users/me/preferences` - View formatting preferences
- `PUT /users/me/preferences` - Update preferences
- `GET /approvals/*` - Check approval status
- `POST /approvals/*/vote` - Vote on approvals (but not their own)

**Blocked Operations**:
- Access to any project data
- Creating new projects
- Inviting members
- Making payments
- All other API endpoints

**UI Experience**:
- Redirected to `/account/pending` (quarantine page)
- Shows approval progress across all projects
- Auto-refreshes every 10 seconds
- Cannot navigate away until approved

---

## Approval Workflow

### Step 1: Security Event Occurs

Example: User changes password

```
1. Password hash updated in database
2. token_version incremented (5 → 6)
3. user.state set to 'PendingApproval'
4. Approval records created in ALL user's projects
5. All project_members.status set to 'pending'
```

### Step 2: Approval Request Created

For each project where user is a member:

```sql
INSERT INTO project_approvals (user_id, project_id, event_type)
VALUES (user_id, project_id, 'password_change');
```

### Step 3: Voting

**Option A: System Admin**
```bash
bonscompte-admin approve username
# Sets user.state = 'active'
# Sets ALL project_members.status = 'active'
# Bypasses voting entirely
```

**Option B: Project Admin**
```
1. Project admin casts 'approve' vote
2. Threshold (1 vote) met instantly
3. project_members.status = 'active' for THAT membership
4. User regains access to THAT project
```

**Option C: Regular Members**
```
1. Members cast 'approve' votes
2. When ceil(33% × member_count) reached:
   - project_members.status = 'active' for THAT membership
   - User regains access to THAT project
```

### Step 4: Resolution

**Approval**:
- `project_approvals.status` → `approved`
- `project_approvals.resolved_at` → current timestamp
- `project_members.status` → `active`
- User can access that project again

**Rejection**:
- `project_approvals.status` → `rejected`
- `project_members.status` remains `pending`
- User stays blocked (only CLI admin can override)

---

## Trust Model & Assumptions

### Trust Assumptions

1. **Project members know each other**: The system assumes members of a project have real-world relationships and can verify identities

2. **Project admins are trustworthy**: Admins have instant approval power within their projects

3. **System admin has CLI access**: Physical/SSH access to server is the ultimate trust boundary

4. **Database integrity**: SQLite database is not tampered with (appropriate file permissions required)

### What This Protects Against

✅ **Compromised password alone cannot maintain access**
- Attacker changes password → approval required
- All existing sessions invalidated

✅ **Stolen token has limited lifespan**
- Token expires in 24 hours
- Invalidated on password change
- Invalidated on token version bump

✅ **Account takeover requires social engineering multiple people**
- Single compromised project member cannot approve alone (unless admin)
- Need 33% of project members to collude

✅ **Brute force is rate-limited**
- Auth endpoints: 5 requests/60s
- General API: 100 requests/second
- Token validation happens server-side

### What This Does NOT Protect Against

❌ **Compromised system admin**
- CLI access allows full account control
- Mitigation: Secure SSH keys, audit `bonscompte-admin` usage

❌ **Majority collusion in small projects**
- 2-person project: 1 vote needed (50% colluded)
- Mitigation: System admin oversight, audit logs

❌ **Low-tech users approving without verification**
- Users may approve without proper identity verification
- Mitigation: User education, clear UI warnings

❌ **Database tampering**
- Direct SQLite write access bypasses all security
- Mitigation: File permissions, regular backups, integrity monitoring

❌ **Server compromise**
- Attacker with root access controls everything
- Mitigation: System hardening, monitoring, regular security updates

---

## Data Isolation

### Project-Scoped Queries

All data access is scoped to projects via the `ProjectMember` extractor:

```rust
// Every project route validates:
1. User is authenticated (valid JWT)
2. User is a member of the project
3. Membership status is 'active' (not 'pending')
4. User has required role (viewer/editor/admin)
```

**Query Pattern**:
```sql
SELECT * FROM payments
WHERE project_id = ? AND <other conditions>
```

**Guarantees**:
- Compromised user can access at most ONE project (if membership active)
- No cross-project data leakage
- No global read/write endpoints

### Immutable Audit Log

All mutations are logged to `history_log` table:

- **Append-only**: Triggers prevent UPDATE/DELETE
- **Hash-chained**: Each entry links to previous entry's hash (SHA256)
- **Tamper-evident**: Chain verification detects modifications
- **Correlation IDs**: Groups related changes (UUID-v4)

**Verification**:
```
GET /projects/{id}/history/verify
```

Returns chain validity and identifies any broken links.

---

## Optional: TOTP (Phase 2)

**Not Yet Implemented**

When enabled:
1. User sets up TOTP (QR code enrollment)
2. Secret stored encrypted in database
3. On approval events:
   - User can self-verify via TOTP code
   - Reduces quorum to 1 OR bypasses approval entirely

**Trade-offs**:
- Reduces human approval burden
- Single factor of compromise (TOTP secret theft)
- Recommended only for tech-savvy users

---

## Monitoring & Incident Response

### Audit Points

Monitor these events for suspicious activity:

1. **Frequent password changes** - May indicate account compromise
2. **Rejected approvals** - Suspicious activity detected by members
3. **CLI admin usage** - Especially `approve` and `revoke` commands
4. **Token version increments** - Track invalidation frequency
5. **Approval voting patterns** - Detect rubber-stamping or collusion

### Incident Response

**Suspected Compromised Account**:
```bash
# 1. Immediately revoke access
bonscompte-admin revoke <username>

# 2. Investigate approval history
SELECT * FROM project_approvals WHERE user_id = <id>;
SELECT * FROM approval_votes WHERE approval_id IN (...);

# 3. Reset password and require re-approval
bonscompte-admin reset-password <username>

# 4. Notify project members via external channel
```

**Suspected Compromised Project Admin**:
```bash
# 1. Demote to regular member (via database or future UI)
UPDATE project_members SET role = 'editor' WHERE user_id = <id>;

# 2. Require approval for role change (future feature)
```

---

## Best Practices

### For Users

1. **Use strong passwords** (minimum 6 chars enforced, 12+ recommended)
2. **Verify identity before approving** - Contact members via known channels (phone, Signal, etc.) before voting
3. **Don't share passwords** - Even with trusted project members
4. **Review approval requests carefully** - Check event type and timing

### For System Admins

1. **Restrict CLI access** - Only trusted individuals should have SSH/shell access
2. **Monitor `bonscompte-admin` usage** - Log all admin commands
3. **Regular backups** - Database includes all approval history
4. **Review rejected approvals** - Investigate why members rejected
5. **Educate users** - Explain approval model and security expectations

### For Project Admins

1. **Vet new members** - Only invite people you trust
2. **Maintain 2+ admins** - Avoid single points of failure
3. **Review approval patterns** - Watch for unusual approval activity
4. **Promote wisely** - Admin role has instant approval power

---

## Roadmap

### Phase 1 (Current)
- ✅ Token versioning and invalidation
- ✅ Password change triggers approval
- ✅ CLI password reset with approval
- ✅ Two-layer approval system
- ✅ Quarantine mode UI
- ✅ Approval voting interface

### Phase 2 (Planned)
- ⏳ Device tracking and new device approvals
- ⏳ TOTP as fast-path approval
- ⏳ Role change approval triggers
- ⏳ Webhook notifications for approvers

### Phase 3 (Future)
- ⏳ Recovery codes (single-use emergency access)
- ⏳ Approval delegation (designate trusted approvers)
- ⏳ Time-based automatic approval (after N days)
- ⏳ Multi-factor authentication (FIDO2/WebAuthn)

---

## Technical Implementation Notes

### Backend

- **Rust** + **Axum 0.8** + **SQLite** + **SQLx**
- **JWT**: `jsonwebtoken` crate
- **Password hashing**: Argon2 (OWASP recommended)
- **Rate limiting**: `tower_governor` (5 req/min auth, 100 req/s general)
- **Migrations**: Inline SQL in `backend/src/db.rs`

### Frontend

- **SvelteKit 2** + **Svelte 5** (runes syntax)
- **TypeScript** for type safety
- **Auto-refresh**: Quarantine page polls every 10s, nav badge every 30s
- **Redirect handling**: 403 + `PENDING_APPROVAL` code triggers `/account/pending`

### Database Schema

```sql
-- User states and token versioning
users (
  id, username, password_hash,
  user_state TEXT CHECK(user_state IN ('active', 'pending_approval', 'revoked')),
  token_version INTEGER DEFAULT 1,
  ...
)

-- Project membership with approval status
project_members (
  id, project_id, user_id, role,
  status TEXT CHECK(status IN ('active', 'pending')),
  ...
)

-- Approval records
project_approvals (
  id, user_id, project_id, event_type,
  status TEXT CHECK(status IN ('pending', 'approved', 'rejected')),
  created_at, resolved_at, ...
)

-- Votes on approvals
approval_votes (
  id, approval_id, voter_id,
  vote TEXT CHECK(vote IN ('approve', 'reject')),
  reason TEXT, voted_at, ...
  UNIQUE(approval_id, voter_id)  -- One vote per person
)
```

---

## References

- **OWASP Top 10**: Mitigates A07:2021 – Identification and Authentication Failures
- **NIST SP 800-63B**: Digital Identity Guidelines (Authentication and Lifecycle Management)
- **FIDO Alliance**: Future integration path for passwordless authentication

---

## Support & Questions

For security concerns or questions about the approval model:
1. Review this document
2. Check the implementation in `backend/src/services/approval_service.rs`
3. Test manually with `cargo test` in backend directory
4. Report issues at https://github.com/anthropics/claude-code/issues (update with actual repo)

---

**Last Updated**: 2025-12-31
**Version**: 1.0.0
**Author**: Claude Code + BonsCompte Team
