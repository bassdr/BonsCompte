# Password Recovery

BonsCompte uses a **human-approved password recovery model** that does not require email or SMTP configuration. This approach prioritizes security over convenience by requiring administrator intervention for password resets.

## Security Model

### Why No Email-Based Recovery?

Traditional email-based password recovery has several vulnerabilities:
- Compromised email accounts can lead to account takeover
- Phishing attacks can trick users into resetting passwords on fake sites
- Email delivery is not guaranteed and can be intercepted

BonsCompte instead uses a **CLI-based recovery** where an administrator must:
1. Generate a temporary password for the user
2. Communicate it through a trusted channel (in person, encrypted messaging, etc.)
3. Approve the user after they've logged in with the temporary password

### User States

| State | Description |
|-------|-------------|
| `active` | Normal user, can access all features |
| `pending_approval` | User has reset password, waiting for admin approval |
| `revoked` | User access has been revoked |

### Token Invalidation

When a password is reset or a user is revoked:
- The `token_version` is incremented in the database
- All existing JWT tokens become invalid immediately
- The user must re-authenticate with the new password

## Administrator Guide

### Prerequisites

You need shell access to the server running BonsCompte. This could be:
- SSH access to a VPS or dedicated server
- Docker exec access to the container
- Direct terminal access

> **Security Warning**: The admin CLI has full control over user accounts. Ensure access to the CLI is properly secured through SSH keys, firewalls, or container security policies. Securing SSH access is outside the scope of this document.

### Running the Admin CLI

#### Docker Deployment (Recommended)

If running BonsCompte via Docker Compose:

```bash
# List all users and their states
docker compose exec backend bonscompte-admin list-users

# Reset a user's password
docker compose exec backend bonscompte-admin reset-password <username>

# Approve a user after password reset
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
# Using cargo alias (recommended)
cargo admin list-users
cargo admin reset-password <username>
cargo admin approve <username>
cargo admin revoke <username>

# Or explicitly
cargo run --bin bonscompte-admin -- list-users
```

#### Direct Binary

If you've built and installed the binary:

```bash
bonscompte-admin list-users
bonscompte-admin reset-password <username>
```

### Password Reset Workflow

1. **User requests reset** (contacts admin through trusted channel)

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

   IMPORTANT: The user must:
     1. Log in with this temporary password
     2. Wait for admin approval
   ```

3. **Admin communicates temporary password** to user through secure channel

4. **User logs in** with temporary password
   - Login succeeds but user sees "Account pending approval" message
   - User cannot access any features until approved

5. **Admin approves user** (after verifying identity through trusted channel):
   ```bash
   docker compose exec backend bonscompte-admin approve alice
   ```

6. **User can now access the application** normally

### Revoking Access

To immediately revoke a user's access:

```bash
docker compose exec backend bonscompte-admin revoke alice
```

This will:
- Set user state to `revoked`
- Increment token version (invalidating all existing sessions)
- Prevent the user from logging in

### Viewing User Status

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

## User Guide

### I Forgot My Password

1. Contact your BonsCompte administrator through a trusted channel (in person, phone, secure messaging)
2. The administrator will generate a temporary password for you
3. Log in with the temporary password at the normal login page
4. You'll see a message that your account is pending approval
5. Wait for the administrator to approve your account
6. Once approved, you can use the application normally

### Why This Process?

This manual process ensures:
- Only verified users can recover their accounts
- No risk of email-based attacks
- Administrators maintain full control over account access
- Clear audit trail of who reset/approved accounts

## Troubleshooting

### "Account pending approval" after login

This is expected after a password reset. Contact your administrator to approve your account.

### "Account revoked" error

Your account has been disabled by an administrator. Contact them to discuss reinstatement.

### "Token expired or invalidated" error

Your session was invalidated, likely due to a password reset or security action. Please log in again with your current password.
