# Symlink Management in DotMe

**Last Updated**: 2026-02-15  
**Module**: `src/symlinks.rs` and `src/dotfiles.rs`

## Overview

DotMe automatically creates and manages symlinks to keep your dotfiles synchronized across your system. All symlinks are tracked in a state file (`~/.dotme/symlinks.yml`) and follow strict safety rules to prevent data loss.

## Table of Contents

1. [Core Rules](#core-rules)
2. [Application in Commands](#application-in-commands)
3. [State File Format](#state-file-format)
4. [Automatic Removal](#automatic-removal)
5. [Implementation Details](#implementation-details)
6. [Examples](#examples)
7. [Testing](#testing)

---

## Core Rules

### Rule 1: Target Does Not Exist
**Condition**: The target path (where the symlink will be created) does not exist at all.

**Action**: Create a symlink pointing from the target path to the source path.

**Example**:
- Source: `~/.dotme/git/dotfiles/.bashrc`
- Target: `~/.bashrc` (does not exist)
- Result: Create symlink `~/.bashrc -> ~/.dotme/git/dotfiles/.bashrc`

### Rule 2: Target Is An Existing Directory
**Condition**: The target path exists and is a directory.

**Action**: Descend into the directory and apply symlink rules recursively to each item in the source directory.

**Example**:
- Source: `~/.dotme/git/dotfiles/.config/` (contains `nvim/`, `tmux/`)
- Target: `~/.config/` (exists as directory)
- Check each item:
  - If `~/.config/nvim` does not exist → create symlink `~/.config/nvim -> ~/.dotme/git/dotfiles/.config/nvim`
  - If `~/.config/tmux` exists (directory) → descend and check its contents
  - If `~/.config/tmux` exists (file) → skip (Rule 3)

### Rule 3: Target Is An Existing File or Symlink
**Condition**: The target path exists as a file or symlink.

**Action**: **NEVER** create a symlink. Skip this item.

**Rationale**: We never overwrite existing files or symlinks to prevent data loss and configuration conflicts.

**Example**:
- Source: `~/.dotme/git/dotfiles/.bashrc`
- Target: `~/.bashrc` (exists as file)
- Result: Skip, do not create symlink

### Rule 4: Every Symlink Must Be Tracked
**Condition**: Any symlink created by DotMe.

**Action**: Add an entry to `~/.dotme/symlinks.yml` with:
- `link`: The path to the symlink
- `target`: The path the symlink points to
- `created_at`: ISO 8601 timestamp when created
- `last_verified`: ISO 8601 timestamp of last verification

**Example Entry**:
```yaml
symlinks:
  - link: "/home/user/.bashrc"
    target: "/home/user/.dotme/git/dotfiles/.bashrc"
    created_at: "2026-02-14T17:45:00Z"
    last_verified: "2026-02-14T17:45:00Z"
```

---

## Application in Commands

### During `dotme add`

When adding a new dotfile entry:

1. **For Files**: 
   - Check if target path exists
   - If not, create symlink from target to source
   - If exists, skip (do not overwrite)

2. **For Directories**:
   - Check if target path exists as directory
   - If not, create symlink from target to source directory
   - If exists as directory, descend and check each item inside

3. **For Git Repositories**:
   - Clone repository to `~/.dotme/git/<repo-name>/`
   - If folders are selected (e.g., `.config`, `.vim`):
     - For each selected folder, process its **contents** (not the folder itself)
   - If no folders selected (sync all):
     - Apply directory rules to entire repository

**Git Repository Folder Behavior**:
```
Repository structure:
  ~/.dotme/git/.dotfiles/geek/
    .geek/
  ~/.dotme/git/.dotfiles/zsh/
    .zshrc
    .oh-my-zsh/

With --folders geek,zsh:
  ~/.geek -> ~/.dotme/git/.dotfiles/geek/.geek
  ~/.zshrc -> ~/.dotme/git/.dotfiles/zsh/.zshrc
  ~/.oh-my-zsh -> ~/.dotme/git/.dotfiles/zsh/.oh-my-zsh
```

### During `dotme update`

When updating all managed dotfiles:

1. **For Files**:
   - Ensure symlink exists from target to source
   - If target exists as file (not symlink), skip
   - If symlink missing, create it

2. **For Directories**:
   - Walk through source directory recursively
   - For each item, check corresponding target path
   - Apply rules 1-3 for each item

3. **For Git Repositories**:
   - Pull latest changes from repository
   - If folders are selected:
     - For each folder, apply directory rules recursively
   - If no folders selected:
     - Apply directory rules to entire repository

### During `dotme remove`

When removing a dotfile entry:

1. **Identify Symlinks**: Find all symlinks belonging to the entry
2. **Remove Symlinks**: Remove from filesystem
3. **Update State**: Remove from `~/.dotme/symlinks.yml`
4. **Clean Resources**: Delete git repository if applicable
5. **Update Config**: Remove entry from config

**Automatic Symlink Detection**:
- **Files**: Matches symlinks pointing to the exact file path
- **Directories**: Matches symlinks pointing to any path inside the directory
- **Git**: Matches symlinks pointing to any path inside the cloned repository

---

## State File Format

### Location
`~/.dotme/symlinks.yml` (default) or custom path set in `paths.symlinks_file`

### Structure
```yaml
symlinks:
  - link: /home/user/.config/nvim/init.vim
    target: /home/user/.dotme/git/dotfiles/.config/nvim/init.vim
    created_at: 2026-02-14T17:45:26.806329089+00:00
    last_verified: 2026-02-14T17:45:26.806329089+00:00
  - link: /home/user/.vimrc
    target: /home/user/dotfiles/.vimrc
    created_at: 2026-02-14T17:45:26.810619601+00:00
    last_verified: 2026-02-14T17:45:26.810619601+00:00
```

### Fields
- **link** (String): Full path to the symlink location
- **target** (String): Full path to the actual file/directory
- **created_at** (DateTime): ISO 8601 timestamp when symlink was created
- **last_verified** (DateTime): ISO 8601 timestamp of last verification

---

## Automatic Removal

### Overview
When removing a dotfile entry with `dotme remove`, all associated symlinks are automatically removed from the filesystem and state file.

### Removal Process

```bash
$ dotme remove https://github.com/user/dotfiles.git
[INFO] Removing associated symlinks...
[INFO]   ✓ Removed symlink: /home/user/.geek
[INFO]   ✓ Removed symlink: /home/user/.zshrc
[INFO]   ✓ Removed symlink: /home/user/.oh-my-zsh
[INFO] ✓ Removed 3 symlink(s)
[INFO] ✓ Git repository deleted
[INFO] ✓ Removed 'https://github.com/user/dotfiles.git' from dotfiles management
```

### Benefits
✅ **Complete Cleanup**: No orphaned symlinks left behind  
✅ **Consistency**: Filesystem state matches tracked state  
✅ **User-Friendly**: Automatic - no manual cleanup needed  
✅ **Safe**: Only removes tracked symlinks  
✅ **Transparent**: Shows exactly what was removed

### Error Handling

**Failed Symlink Removal**:
```bash
[WARN]   ✗ Failed to remove symlink /home/user/.geek: Permission denied
```
- Logs warning but continues with other symlinks
- Doesn't fail the entire remove operation

**Missing Symlinks**:
- If a symlink in state file doesn't exist on filesystem
- Handled gracefully with no errors
- State file is updated anyway

---

## Implementation Details

### Recursive Directory Processing

When processing a directory (source exists, target exists as directory):

```rust
async fn process_directory_for_symlinks(source_dir, target_dir) {
    for each item in source_dir:
        source_item = source_dir / item
        target_item = target_dir / item
        
        if target_item does not exist:
            // Rule 1: Create symlink
            create_symlink(target_item -> source_item)
            track_symlink(target_item, source_item)
        
        else if target_item is directory:
            if source_item is directory:
                // Rule 2: Descend recursively
                process_directory_for_symlinks(source_item, target_item)
            else:
                // source is file, target is dir - skip
                skip()
        
        else:
            // Rule 3: Target exists as file/symlink - skip
            skip()
}
```

### Async Recursion
Used `Box::pin()` for recursive async function calls to avoid infinite size futures:

```rust
Box::pin(process_directory_for_symlinks(&source_path, &target_path)).await?;
```

### Cross-platform Support
Symlink creation supports both Unix and Windows through conditional compilation:

```rust
#[cfg(unix)]
fs::symlink(target, link).await?;

#[cfg(windows)]
{
    if target.is_dir() {
        fs::symlink_dir(target, link).await?;
    } else {
        fs::symlink_file(target, link).await?;
    }
}
```

### Special Cases

#### Hidden Files
- **Rule**: Process hidden files (starting with `.`) the same as regular files
- **Example**: `.bashrc`, `.gitconfig`, etc. are all processed normally

#### `.git` Directory
- **Rule**: Skip the `.git` directory in git repositories
- **Rationale**: The `.git` directory is repository metadata, not dotfiles

#### Symlink Verification
- Before creating a symlink, verify the source exists
- After creating a symlink, verify it points to the correct target
- During updates, verify existing symlinks still point to correct targets

#### Broken Symlinks
- If a target path is a broken symlink (points to non-existent location):
  - Treat as "exists" (Rule 3) - do not overwrite
  - Log a warning
  - User must manually remove broken symlinks

### Safety Guarantees

1. **No Data Loss**: Never overwrites existing files or directories
2. **Idempotent**: Running `update` multiple times is safe
3. **Traceable**: All symlinks tracked in state file
4. **Reversible**: State file enables complete cleanup operations
5. **Verified**: Checks filesystem state before operations

---

## Examples

### Example 1: Fresh Installation
```
Source: ~/.dotme/git/dotfiles/
  .bashrc
  .vimrc
  .config/
    nvim/
      init.vim

Target: ~/
  (empty home directory)

Result:
  ~/.bashrc -> ~/.dotme/git/dotfiles/.bashrc
  ~/.vimrc -> ~/.dotme/git/dotfiles/.vimrc
  ~/.config/nvim -> ~/.dotme/git/dotfiles/.config/nvim
```

### Example 2: Partial Existing Config
```
Source: ~/.dotme/git/dotfiles/
  .bashrc
  .vimrc
  .config/
    nvim/
      init.vim

Target: ~/
  .bashrc (existing file)
  .config/ (existing directory)
    (empty)

Result:
  ~/.bashrc (skip - exists)
  ~/.vimrc -> ~/.dotme/git/dotfiles/.vimrc
  ~/.config/nvim -> ~/.dotme/git/dotfiles/.config/nvim
```

### Example 3: Nested Existing Directories
```
Source: ~/.dotme/git/dotfiles/
  .config/
    nvim/
      init.vim
      plugins.vim

Target: ~/
  .config/ (exists)
    nvim/ (exists)
      init.vim (exists)

Result:
  ~/.config/nvim/init.vim (skip - exists)
  ~/.config/nvim/plugins.vim -> ~/.dotme/git/dotfiles/.config/nvim/plugins.vim
```

### Example 4: Git Repository with Selective Folders
```bash
$ dotme add https://github.com/GeekMasher/.dotfiles.git --folders geek,dev,zsh
```

```
Repository structure:
  ~/.dotme/git/.dotfiles/geek/
    .geek/
  ~/.dotme/git/.dotfiles/dev/
    .local/
      dev/
      edit/
  ~/.dotme/git/.dotfiles/zsh/
    .zshrc
    .oh-my-zsh/

Result:
  ~/.geek -> ~/.dotme/git/.dotfiles/geek/.geek
  ~/.local/dev -> ~/.dotme/git/.dotfiles/dev/.local/dev
  ~/.local/edit -> ~/.dotme/git/.dotfiles/dev/.local/edit
  ~/.zshrc -> ~/.dotme/git/.dotfiles/zsh/.zshrc
  ~/.oh-my-zsh -> ~/.dotme/git/.dotfiles/zsh/.oh-my-zsh
```

### Example 5: Removing Entry with Symlinks
```bash
# Add entry
$ dotme add ~/dotfiles/.config
[INFO] Creating symlink: "/home/user/.config/nvim/init.vim" -> "..."
[INFO] Creating symlink: "/home/user/.config/nvim/plugins.vim" -> "..."
[INFO] Creating symlink: "/home/user/.config/tmux" -> "..."

# Remove entry
$ dotme remove ~/dotfiles/.config
[INFO] Removing associated symlinks...
[INFO]   ✓ Removed symlink: /home/user/.config/nvim/init.vim
[INFO]   ✓ Removed symlink: /home/user/.config/nvim/plugins.vim
[INFO]   ✓ Removed symlink: /home/user/.config/tmux
[INFO] ✓ Removed 3 symlink(s)
[INFO] ✓ Removed '~/dotfiles/.config' from dotfiles management
```

---

## Testing

### Integration Testing

**Setup Test Environment**:
```bash
mkdir -p testing/test-dotfiles/.config/nvim
echo "test" > testing/test-dotfiles/.vimrc
echo "init" > testing/test-dotfiles/.config/nvim/init.vim
```

**Test Add Command**:
```bash
$ cargo run -- -c ./testing/config.yml add testing/test-dotfiles
[INFO] Creating symlink: "/home/user/.vimrc" -> ".../testing/test-dotfiles/.vimrc"
[INFO] Creating symlink: "/home/user/.config/nvim/init.vim" -> "..."
```

**Verify State File**:
```bash
$ cat ~/.dotme/symlinks.yml
symlinks:
  - link: /home/user/.vimrc
    target: /home/user/dotme/testing/test-dotfiles/.vimrc
    created_at: 2026-02-14T17:45:26.810619601+00:00
    last_verified: 2026-02-14T17:45:26.810619601+00:00
```

**Test Update Command**:
```bash
$ cargo run -- -c ./testing/config.yml update
[INFO] Verifying symlinks...
[INFO] ✓ All symlinks verified
```

**Test Remove Command**:
```bash
$ cargo run -- -c ./testing/config.yml remove testing/test-dotfiles
[INFO] Removing associated symlinks...
[INFO]   ✓ Removed symlink: /home/user/.vimrc
[INFO]   ✓ Removed symlink: /home/user/.config/nvim/init.vim
[INFO] ✓ Removed 2 symlink(s)
```

### Test Cases

✅ **Empty home directory**: All symlinks created  
✅ **Existing files**: Skipped (Rule 3)  
✅ **Nested directories**: Recursive processing works  
✅ **Broken symlinks**: Detected and skipped  
✅ **Git repositories**: Folder contents processed correctly  
✅ **Symlink removal**: All associated symlinks removed  
✅ **State persistence**: Tracked correctly in YAML file

### Unit Tests
```bash
$ cargo test
```

**Tests**:
- `test_symlink_state_default` - State file initialization
- `test_add_entry` - Adding symlink entries
- `test_remove_entry` - Removing symlink entries
- `test_find_entry` - Finding symlink entries

---

## Logging

### Debug Level
```bash
$ cargo run -- --debug add ~/dotfiles
[DEBUG] Checking if target exists: /home/user/.bashrc
[DEBUG] Target exists, skipping: /home/user/.bashrc
[DEBUG] Target does not exist, creating symlink: /home/user/.vimrc
```

### Info Level (Default)
```bash
$ cargo run -- add ~/dotfiles
[INFO] Creating symlink: "/home/user/.vimrc" -> "/home/user/dotfiles/.vimrc"
[INFO] ✓ Created symlink: /home/user/.vimrc -> /home/user/dotfiles/.vimrc
[INFO] Skipping (exists): /home/user/.bashrc
```

### Dry Run Mode
```bash
$ cargo run -- add ~/dotfiles --dry-run
[DRY RUN] Would create symlink: /home/user/.vimrc -> /home/user/dotfiles/.vimrc
[DRY RUN] Would skip (exists): /home/user/.bashrc
```

---

## Future Enhancements

Possible future improvements:

1. **Symlink verification command**: `dotme verify-symlinks`
   - Check all tracked symlinks are valid
   - Identify broken symlinks
   - Suggest repairs

2. **Broken symlink cleanup**: `dotme cleanup-symlinks`
   - Remove broken symlinks from state file
   - Optionally remove from filesystem

3. **Symlink status in status command**: Show symlink health
   - Count of valid/broken symlinks
   - Last verification time

4. **Interactive conflict resolution**: Prompt user when file exists
   - Backup existing file
   - Skip or overwrite options

5. **Symlink migration**: Convert existing files to symlinks
   - Detect files that should be symlinked
   - Backup and replace with symlinks

---

## References

- **Module**: `src/symlinks.rs` - Symlink state management
- **Module**: `src/dotfiles.rs` - Symlink creation and removal logic
- **Config**: `~/.dotme/symlinks.yml` - Symlink state file
- **Testing**: `./testing/config.yml` - Integration test configuration
