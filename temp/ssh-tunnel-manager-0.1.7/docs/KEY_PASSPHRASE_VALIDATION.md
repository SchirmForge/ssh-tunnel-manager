# SSH Key Passphrase Validation

## Problem

Previously, when adding a profile with SSH key authentication, the CLI would store the passphrase in the system keychain without validating it. This could lead to:

1. **Invalid passphrases stored** - User makes a typo, wrong passphrase gets saved
2. **Failed connections** - Tunnel fails to start with stored invalid passphrase
3. **Poor user experience** - No immediate feedback, error only appears later

## Solution

Added passphrase validation during profile creation by attempting to decrypt the SSH key before storing the passphrase in the keychain.

### Implementation

**New function** ([main.rs:778-790](crates/cli/src/main.rs#L778-L790)):
```rust
fn validate_key_passphrase(key_path: &PathBuf, passphrase: &str) -> Result<()> {
    use russh_keys::decode_secret_key;

    // Read the key file
    let key_data = fs::read_to_string(key_path)
        .context("Failed to read SSH key file")?;

    // Attempt to decode with the passphrase
    decode_secret_key(&key_data, Some(passphrase))
        .map_err(|e| anyhow::anyhow!("Invalid passphrase: {}", e))?;

    Ok(())
}
```

**Updated profile creation flow** ([main.rs:509-525](crates/cli/src/main.rs#L509-L525)):
```rust
if store_passphrase {
    let passphrase = Password::new()
        .with_prompt("SSH key passphrase")
        .interact()?;

    // Validate the passphrase by attempting to load the key
    if let Err(e) = validate_key_passphrase(&key_path, &passphrase) {
        println!("⚠️  Failed to load SSH key with provided passphrase: {}", e);
        println!("The passphrase will not be stored.");
        (AuthType::Key, Some(key_path), false)
    } else {
        store_password_in_keychain(&profile_id, &passphrase)?;
        (AuthType::Key, Some(key_path), store_passphrase)
    }
}
```

## Behavior

### Non-Interactive Mode with Encrypted Key
```bash
$ ssh-tunnel add test1 -H 192.168.122.217 -y -u user1 -k ~/.ssh/id_user1
SSH key is encrypted and requires a passphrase.
SSH key passphrase: ********
Store passphrase in system keychain? [y/N] y
  ✓ Password stored in system keychain
✓ Profile created successfully!
```

**Note:** When an encrypted key is detected, you're **always** prompted for the passphrase and asked whether to store it, even with the `-y` flag. This is necessary because:
1. The key cannot be used without the passphrase
2. You're already providing input interactively for the passphrase
3. You should have control over whether it's stored

The default is `N` (don't store) in non-interactive mode for security, but you can choose `y` to store it.

### Interactive Mode with Encrypted Key (Valid)
```bash
$ ssh-tunnel add myserver -k ~/.ssh/id_ed25519
...
Does this key have a passphrase you want to store in keychain? [y/N] y
SSH key passphrase: ********
  ✓ Password stored in system keychain
✓ Profile created successfully!
```

### Interactive Mode with Encrypted Key (Invalid)
```bash
$ ssh-tunnel add myserver -k ~/.ssh/id_ed25519
...
Does this key have a passphrase you want to store in keychain? [y/N] y
SSH key passphrase: ********
⚠️  Failed to load SSH key with provided passphrase: Invalid passphrase: ...
The passphrase will not be stored.
✓ Profile created successfully!
```

The profile is still created, but the passphrase is **not** stored. The user will be prompted for the correct passphrase when starting the tunnel.

### Unencrypted Key
```bash
$ ssh-tunnel add myserver -k ~/.ssh/id_unencrypted
...
✓ Profile created successfully!
```

No passphrase prompt - the tool detects the key is not encrypted.

### SSH Password Authentication
For password authentication (not key-based), we can't validate without connecting to the server:

```
$ ssh-tunnel add myserver
...
Store password in system keychain? [y/N] y
⚠️  Note: Password cannot be validated until first connection.
    If the password is incorrect, you'll be prompted again when starting the tunnel.
  ✓ Password stored in system keychain
```

## Benefits

1. **Immediate feedback** - User knows right away if passphrase is wrong
2. **Prevents invalid storage** - Only correct passphrases are stored
3. **Better UX** - Clear error messages guide the user
4. **No data loss** - Profile is still created even if passphrase is invalid
5. **Secure** - Uses the same `russh_keys` library the daemon uses

## Technical Details

### Key Encryption Detection
- `is_key_encrypted()` attempts to load the key without a passphrase
- If it loads successfully → key is unencrypted
- If it fails → key is encrypted (needs passphrase)

### Passphrase Validation
- Uses `russh_keys::decode_secret_key()` to validate the passphrase
- Supports all SSH key formats supported by `russh_keys`:
  - RSA
  - Ed25519
  - ECDSA
  - Encrypted/unencrypted keys
- Graceful error handling - invalid passphrase doesn't abort profile creation
- Added `russh-keys` dependency to CLI crate

### Non-Interactive Mode Behavior
- Detects encrypted keys automatically
- **Always prompts** for passphrase if key is encrypted (even with `-y`)
- **Always asks** whether to store the passphrase (even with `-y`)
- Default is `N` (don't store) in non-interactive mode, `Y` in interactive mode
- User has full control over keychain storage

## Future Improvements

- Could offer to retry passphrase input on validation failure
- Could add an option to test password authentication (requires server connection)
- Could validate stored passphrases when editing profiles
