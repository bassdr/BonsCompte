# Password Recovery

BonsCompte uses a **two-layer human-approved password recovery model** that does not require email or SMTP configuration. This approach prioritizes security by requiring both administrator intervention and project member approval for password resets.

## Security Model

### Why No Email-Based Recovery?

Traditional email-based password recovery has several vulnerabilities:
- Compromised email accounts can lead to account takeover
- Phishing attacks can trick users into resetting passwords on fake sites
- Email delivery is not guaranteed and can be intercepted

BonsCompte instead uses a **two-layer approval system**:

1. **System Administrator (CLI)**: Generates temporary password and can fully restore account access
2. **Project Members**: Vote to approve access to their specific projects

### Two-Layer Approval System

#### Layer 1: System Administrator

**Who**: Users with CLI access to the server

**Powers**:
- Generate temporary passwords
- Fully restore global account state
- Activate ALL project memberships
- Emergency override for any approval

**Use Cases**:
- Password resets
- Account recovery for users locked out of all projects
- Solo projects (where user is the only member)

#### Layer 2: Project Member Voting

**Who**: Active members of projects where the user participates

**Voting Thresholds**:
- **Project Admins**: 1 vote (instant approval for that project)
- **Regular Members**: ceil(33% × member_count) votes required
- **Solo Projects**: Only system admin can approve (users can't self-approve)

**Effects**:
- Approves access to ONE specific project
- Does NOT affect other projects (each must be approved independently)

### User States

| State | Description |
|-------|-------------|
| `active` | Normal user, can access projects with active memberships |
| `pending_approval` | User has reset password, waiting for approval |
| `revoked` | User access has been revoked entirely |

### Project Membership States

| Status | Description |
|--------|-------------|
| `active` | User can access this project |
| `pending` | User is awaiting approval to access this project |

### Token Invalidation

When a password is reset or changed:
- The `token_version` is incremented in the database
- All existing JWT tokens become invalid **immediately**
- The user must re-authenticate with the new password
- All project memberships are set to `pending`

---

## Administrator Guide

### Prerequisites

You need shell access to the server running BonsCompte:
- SSH access to a VPS or dedicated server
- Docker exec access to the container
- Direct terminal access

> **Security Warning**: The admin CLI has full control over user accounts. Ensure access is secured through SSH keys, firewalls, or container security policies.

### Running the Admin CLI

#### Docker Deployment (Recommended)

If running BonsCompte via Docker Compose:

```bash
# List all users and their states
docker compose exec backend bonscompte-admin list-users

# Reset a user's password
docker compose exec backend bonscompte-admin reset-password <username>

# Approve a user (full global restoration)
docker compose exec backend bonscompte-admin approve <username>

# Revoke a user's access
docker compose exec backend bonscompte-admin revoke <username>
```

If using standalone Docker:

```bash
# Find the container name/ID
docker ps | grep bonscompte-backend

# Run admin commands
docker exec -it <container_id> bonscompte-admin list-users
docker exec -it <container_id> bonscompte-admin reset-password <username>
```

#### Local Development

From the `backend/` directory:

```bash
# Using cargo (development)
cargo run --bin bonscompte-admin -- list-users
cargo run --bin bonscompte-admin -- reset-password <username>
cargo run --bin bonscompte-admin -- approve <username>
cargo run --bin bonscompte-admin -- revoke <username>

# Using built binary
./target/debug/bonscompte-admin list-users
```

#### Direct Binary (Production)

If you've built and installed the binary:

```bash
bonscompte-admin list-users
bonscompte-admin reset-password <username>
bonscompte-admin approve <username>
bonscompte-admin revoke <username>
```

---

## Password Reset Workflows

### Option A: Full Admin Restoration (Fastest)

Use this when:
- User is locked out of all projects
- User is in solo projects
- Emergency access needed

**Steps**:

1. **User requests reset** (contacts admin through trusted channel: phone, Signal, in-person)

2. **Admin generates temporary password**:
   ```bash
   docker compose exec backend bonscompte-admin reset-password alice
   ```

   Output:
   ```
   Password reset for user 'alice'
   Temporary password: Abc123!@DefGhi
   Previous state: active
   New state: pending_approval
   Token version: 1 -> 2
   Project memberships set to pending: 3

   IMPORTANT: The user must:
     1. Log in with this temporary password
     2. Wait for admin approval (run: bonscompte-admin approve alice)
   ```

3. **Admin securely communicates** temporary password to user

4. **User logs in** with temporary password
   - Login succeeds
   - Redirected to "Account Pending Approval" page
   - Can only view approval status

5. **Admin approves user** (after verifying identity):
   ```bash
   docker compose exec backend bonscompte-admin approve alice
   ```

   Output:
   ```
   User 'alice' approved
   Previous state: pending_approval
   New state: active
   Token version: 2 (unchanged)
   Project memberships activated: 3
   ```

6. **User can now access ALL projects** normally

### Option B: Project Member Voting (Decentralized)

Use this when:
- Multiple people can verify the user's identity
- User is a member of active projects with other members
- Want to distribute approval responsibility

**Steps**:

1. **User requests reset** (contacts admin)

2. **Admin generates temporary password**:
   ```bash
   docker compose exec backend bonscompte-admin reset-password alice
   ```

3. **Admin communicates** temporary password to user

4. **User logs in** with temporary password
   - Sees "Account Pending Approval" page
   - Shows approval status for each project

5. **Project members vote**:
   - Project admins: Can instantly approve with 1 vote
   - Regular members: Need ceil(33% × member_count) votes
   - Members navigate to **Approvals** in the UI
   - Click **Approve** or **Reject** with optional reason

6. **User regains access** project-by-project as approvals are granted

7. *(Optional)* **Admin can bypass voting** if needed:
   ```bash
   docker compose exec backend bonscompte-admin approve alice
   ```

### Comparison

| Aspect | Admin Restoration | Project Voting |
|--------|-------------------|----------------|
| **Speed** | Instant (1 command) | Depends on members responding |
| **Decentralization** | Single point (admin) | Distributed (33% quorum) |
| **Use Case** | Emergency, solo projects | Normal operations, active projects |
| **Projects Activated** | ALL | One at a time (as approved) |

---

## Viewing User & Approval Status

### List All Users

```bash
docker compose exec backend bonscompte-admin list-users
```

Output:
```
ID    Username             Display Name         State              TokVer   Created
------------------------------------------------------------------------------------------
1     david                David                active             1        2025-12-15 17:47:47
2     alice                Alice                pending_approval   2        2025-12-16 10:30:00
3     bob                  Bob                  revoked            3        2025-12-17 09:15:22
```

### Check Approval Status (via database)

```bash
docker compose exec backend sqlite3 /data/data.db
```

```sql
-- View pending approvals
SELECT
    pa.id,
    pa.event_type,
    u.username,
    p.name as project_name,
    pa.status,
    pa.created_at
FROM project_approvals pa
JOIN users u ON pa.user_id = u.id
JOIN projects p ON pa.project_id = p.id
WHERE pa.status = 'pending';

-- View votes on an approval
SELECT
    av.vote,
    u.username as voter,
    av.voted_at,
    av.reason
FROM approval_votes av
JOIN users u ON av.voter_id = u.id
WHERE av.approval_id = <approval_id>;
```

---

## Revoking Access

To immediately revoke a user's access:

```bash
docker compose exec backend bonscompte-admin revoke alice
```

Output:
```
User 'alice' revoked
Previous state: active
New state: revoked
Token version: 2 -> 3 (all tokens invalidated)
```

**Effects**:
- User state set to `revoked`
- Token version incremented → all sessions invalidated
- User cannot log in (revoked accounts are blocked at login)

**To restore**: Reset password again and approve

---

## User Guide (End-User Facing)

### I Forgot My Password

**See**: `/help/password-recovery` in the app (available in English and French)

**Quick Steps**:

1. **Contact administrator** through a trusted channel (phone, Signal, in-person)
2. **Receive temporary password** from administrator
3. **Log in** with temporary password
4. **Wait for approval**:
   - System admin can approve immediately (full access)
   - OR project members can vote (project-by-project access)
5. **Access restored** once approved

### Why This Process?

- **No email vulnerabilities**: Can't be phished or intercepted
- **Identity verification**: Must contact someone who knows you
- **Audit trail**: All resets and approvals are logged
- **Distributed trust**: Project members validate access, not just one admin

---

## Troubleshooting

### "Account pending approval" after login

**Cause**: Password was reset, waiting for approval

**Solution**:
- **For user**: Wait for project members to vote OR contact system admin
- **For admin**: Run `bonscompte-admin approve <username>` to bypass voting

### "Account revoked" error

**Cause**: Account was disabled by administrator

**Solution**: Contact administrator to discuss reinstatement

### "Token expired or invalidated" error

**Cause**: Session invalidated due to password change or security action

**Solution**: Log in again with current password

### User can't access a specific project after approval

**Possible causes**:
1. **Project membership is still pending**: That project hasn't approved yet
2. **User not a member**: Check project member list
3. **Admin used `reset-password` but hasn't run `approve`**: Admin must run approve command

**Solution**:
- **For user**: Ask project admin or members to vote
- **For admin**: Run `bonscompte-admin approve <username>` to activate all memberships

### Solo project - can't self-approve

**Cause**: Single-user projects prevent self-approval for security

**Solution**: Only system admin can approve via CLI

---

## Security Considerations

### Best Practices

1. **Secure CLI Access**: Protect server/SSH access with strong keys and firewall rules
2. **Verify Identity**: Always verify user identity before resetting passwords
3. **Use Secure Channels**: Share temporary passwords via encrypted messaging (Signal, Matrix) or in-person
4. **Monitor Admin Usage**: Log all `bonscompte-admin` commands
5. **Educate Users**: Explain the approval process and why it's important

### Threat Model

**What This Protects Against**:
- ✅ Email-based account takeover
- ✅ Compromised password alone (requires approval to continue)
- ✅ Stolen JWT tokens (invalidated on password change)
- ✅ Single malicious project member (requires quorum)

**What This Does NOT Protect Against**:
- ❌ Compromised system administrator (full control)
- ❌ Majority collusion in small projects (1 vote needed in 2-person projects)
- ❌ Direct database tampering
- ❌ Server compromise (attacker with root access)

### Audit Trail

All password recovery events are logged:

1. **CLI commands**: System logs (stdout/stderr)
2. **Approval creation**: `project_approvals` table
3. **Votes**: `approval_votes` table
4. **Token invalidations**: User `token_version` increments

**Recommended monitoring**:
- Track admin CLI usage
- Alert on rejected approvals
- Monitor frequent password resets

---

## Related Documentation

- **docs/SECURITY.md**: Comprehensive security model documentation
- **In-App Help**: `/help/password-recovery` (user-facing guide, translated)
- **CLAUDE.md**: Project architecture and development guide

---

**Last Updated**: 2025-12-31
**Version**: 2.0.0 (Two-Layer Approval System)
