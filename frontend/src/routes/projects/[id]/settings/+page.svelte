<script lang="ts">
    import { page } from '$app/stores';
    import { goto } from '$app/navigation';
    import { updateProject, deleteProject, regenerateInviteCode } from "$lib/api";
    import { currentProject, loadProject, isAdmin } from '$lib/stores/project';

    let error = $state('');
    let success = $state('');

    let projectId = $derived(parseInt($page.params.id ?? ''));

    // Edit form
    let name = $state('');
    let description = $state('');
    let saving = $state(false);

    // Delete confirmation
    let showDeleteConfirm = $state(false);
    let deleteConfirmText = $state('');
    let deleting = $state(false);

    // Initialize form when project loads
    $effect(() => {
        if ($currentProject) {
            name = $currentProject.name;
            description = $currentProject.description || '';
        }
    });

    async function handleSave(e: Event) {
        e.preventDefault();
        if (!name.trim()) return;

        saving = true;
        error = '';
        success = '';

        try {
            await updateProject(projectId, {
                name: name.trim(),
                description: description.trim() || undefined
            });

            success = 'Project updated successfully';
            await loadProject(projectId);
        } catch (e) {
            error = e instanceof Error ? e.message : 'Failed to update project';
        } finally {
            saving = false;
        }
    }

    async function handleRegenerateCode() {
        if (!confirm('Generate a new invite code? The old code will stop working.')) return;

        error = '';
        success = '';

        try {
            await regenerateInviteCode(projectId);
            success = 'Invite code regenerated';
            await loadProject(projectId);
        } catch (e) {
            error = e instanceof Error ? e.message : 'Failed to regenerate code';
        }
    }

    async function handleDelete() {
        if (deleteConfirmText !== $currentProject?.name) {
            error = 'Project name does not match';
            return;
        }

        deleting = true;
        error = '';

        try {
            await deleteProject(projectId);
            goto('/');
        } catch (e) {
            error = e instanceof Error ? e.message : 'Failed to delete project';
            deleting = false;
        }
    }
</script>

<h2>Project Settings</h2>

{#if !$isAdmin}
    <div class="error">You need admin permissions to access settings.</div>
{:else}
    {#if error}
        <div class="error">{error}</div>
    {/if}

    {#if success}
        <div class="success">{success}</div>
    {/if}

    <section class="card">
        <h3>Project Details</h3>
        <form onsubmit={handleSave}>
            <div class="field">
                <label for="name">Project Name</label>
                <input
                    id="name"
                    type="text"
                    bind:value={name}
                    required
                />
            </div>
            <div class="field">
                <label for="description">Description</label>
                <textarea
                    id="description"
                    bind:value={description}
                    rows="3"
                    placeholder="Optional description"
                ></textarea>
            </div>
            <button type="submit" disabled={saving}>
                {saving ? 'Saving...' : 'Save Changes'}
            </button>
        </form>
    </section>

    <section class="card">
        <h3>Invite Code</h3>
        <p>Share this code with others to let them join the project.</p>
        <div class="invite-row">
            <code>{$currentProject?.invite_code}</code>
            <button class="btn-secondary" onclick={handleRegenerateCode}>
                Regenerate Code
            </button>
        </div>
    </section>

    <section class="card danger-zone">
        <h3>Danger Zone</h3>
        {#if !showDeleteConfirm}
            <p>Deleting the project will permanently remove all payments, participants, and members.</p>
            <button class="btn-danger" onclick={() => showDeleteConfirm = true}>
                Delete Project
            </button>
        {:else}
            <p>Type the project name to confirm deletion: <strong>{$currentProject?.name}</strong></p>
            <div class="delete-confirm">
                <input
                    type="text"
                    bind:value={deleteConfirmText}
                    placeholder="Project name"
                />
                <button
                    class="btn-danger"
                    onclick={handleDelete}
                    disabled={deleting || deleteConfirmText !== $currentProject?.name}
                >
                    {deleting ? 'Deleting...' : 'Confirm Delete'}
                </button>
                <button class="btn-secondary" onclick={() => { showDeleteConfirm = false; deleteConfirmText = ''; }}>
                    Cancel
                </button>
            </div>
        {/if}
    </section>
{/if}

<style>
    h2 {
        margin-bottom: 1.5rem;
    }

    .card {
        background: rgba(255, 255, 255, 0.8);
        backdrop-filter: blur(10px);
        border-radius: 16px;
        padding: 1.5rem;
        margin-bottom: 1.5rem;
        box-shadow: 0 4px 20px rgba(0, 0, 0, 0.05);
    }

    h3 {
        margin-top: 0;
        margin-bottom: 1rem;
        color: var(--accent, #7b61ff);
    }

    .error {
        background: #fee;
        color: #c00;
        padding: 0.75rem;
        border-radius: 8px;
        margin-bottom: 1rem;
    }

    .success {
        background: #d4edda;
        color: #155724;
        padding: 0.75rem;
        border-radius: 8px;
        margin-bottom: 1rem;
    }

    .field {
        margin-bottom: 1rem;
    }

    label {
        display: block;
        margin-bottom: 0.5rem;
        font-weight: 600;
        font-size: 0.9rem;
    }

    input, textarea {
        width: 100%;
        padding: 0.75rem;
        border: 1px solid #ddd;
        border-radius: 8px;
        font-size: 1rem;
        font-family: inherit;
    }

    input:focus, textarea:focus {
        outline: none;
        border-color: var(--accent, #7b61ff);
    }

    button[type="submit"] {
        padding: 0.75rem 1.5rem;
        background: var(--accent, #7b61ff);
        color: white;
        border: none;
        border-radius: 8px;
        font-size: 1rem;
        cursor: pointer;
    }

    button[type="submit"]:disabled {
        opacity: 0.6;
        cursor: not-allowed;
    }

    .invite-row {
        display: flex;
        align-items: center;
        gap: 1rem;
    }

    .invite-row code {
        background: #f5f5f5;
        padding: 1rem 1.5rem;
        border-radius: 8px;
        font-size: 1.2rem;
        font-weight: 600;
        letter-spacing: 0.1em;
    }

    .btn-secondary {
        padding: 0.75rem 1.5rem;
        background: white;
        color: var(--accent, #7b61ff);
        border: 2px solid var(--accent, #7b61ff);
        border-radius: 8px;
        font-size: 1rem;
        cursor: pointer;
    }

    .btn-secondary:hover {
        background: rgba(123, 97, 255, 0.1);
    }

    .danger-zone {
        border: 2px solid #f5c6cb;
    }

    .danger-zone h3 {
        color: #c00;
    }

    .danger-zone p {
        color: #666;
        margin-bottom: 1rem;
    }

    .btn-danger {
        padding: 0.75rem 1.5rem;
        background: #dc3545;
        color: white;
        border: none;
        border-radius: 8px;
        font-size: 1rem;
        cursor: pointer;
    }

    .btn-danger:hover:not(:disabled) {
        background: #c82333;
    }

    .btn-danger:disabled {
        opacity: 0.6;
        cursor: not-allowed;
    }

    .delete-confirm {
        display: flex;
        gap: 0.5rem;
        align-items: center;
    }

    .delete-confirm input {
        flex: 1;
    }
</style>
