<script lang="ts">
    import { page } from '$app/stores';
    import { goto } from '$app/navigation';
    import {
        updateProject,
        deleteProject,
        regenerateInviteCode,
        updateProjectSettings,
        getPendingMembers,
        approveMember,
        rejectMember,
        type ProjectMember
    } from "$lib/api";
    import { currentProject, loadProject, isAdmin } from '$lib/stores/project';
    import { _ } from '$lib/i18n';

    let error = $state('');
    let success = $state('');

    let projectId = $derived(parseInt($page.params.id ?? ''));

    // Edit form
    let name = $state('');
    let description = $state('');
    let saving = $state(false);

    // Invite settings
    let invitesEnabled = $state(true);
    let requireApproval = $state(false);
    let savingSettings = $state(false);

    // Pending members
    let pendingMembers = $state<ProjectMember[]>([]);
    let loadingPending = $state(false);
    let processingMember = $state<number | null>(null);

    // Delete confirmation
    let showDeleteConfirm = $state(false);
    let deleteConfirmText = $state('');
    let deleting = $state(false);

    // Initialize form when project loads
    $effect(() => {
        if ($currentProject) {
            name = $currentProject.name;
            description = $currentProject.description || '';
            invitesEnabled = $currentProject.invites_enabled;
            requireApproval = $currentProject.require_approval;
        }
    });

    // Load pending members when project changes
    $effect(() => {
        if (projectId && $isAdmin) {
            loadPendingMembers();
        }
    });

    async function loadPendingMembers() {
        loadingPending = true;
        try {
            pendingMembers = await getPendingMembers(projectId);
        } catch (e) {
            console.error('Failed to load pending members:', e);
        } finally {
            loadingPending = false;
        }
    }

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

    async function handleSaveSettings() {
        savingSettings = true;
        error = '';
        success = '';

        try {
            await updateProjectSettings(projectId, {
                invites_enabled: invitesEnabled,
                require_approval: requireApproval
            });
            success = 'Invite settings updated';
            await loadProject(projectId);
        } catch (e) {
            error = e instanceof Error ? e.message : 'Failed to update settings';
        } finally {
            savingSettings = false;
        }
    }

    async function handleApproveMember(userId: number) {
        processingMember = userId;
        error = '';

        try {
            await approveMember(projectId, userId);
            await loadPendingMembers();
            success = 'Member approved';
        } catch (e) {
            error = e instanceof Error ? e.message : 'Failed to approve member';
        } finally {
            processingMember = null;
        }
    }

    async function handleRejectMember(userId: number) {
        if (!confirm('Are you sure you want to reject this member request?')) return;

        processingMember = userId;
        error = '';

        try {
            await rejectMember(projectId, userId);
            await loadPendingMembers();
            success = 'Member rejected';
        } catch (e) {
            error = e instanceof Error ? e.message : 'Failed to reject member';
        } finally {
            processingMember = null;
        }
    }
</script>

<h2>{$_('projectSettings.title')}</h2>

{#if !$isAdmin}
    <div class="error">{$_('projectSettings.needAdminPermissions')}</div>
{:else}
    {#if error}
        <div class="error">{error}</div>
    {/if}

    {#if success}
        <div class="success">{success}</div>
    {/if}

    <section class="card">
        <h3>{$_('projectSettings.projectDetails')}</h3>
        <form onsubmit={handleSave}>
            <div class="field">
                <label for="name">{$_('projectSettings.projectName')}</label>
                <input
                    id="name"
                    type="text"
                    bind:value={name}
                    required
                />
            </div>
            <div class="field">
                <label for="description">{$_('projectSettings.description')}</label>
                <textarea
                    id="description"
                    bind:value={description}
                    rows="3"
                    placeholder={$_('projectSettings.descriptionPlaceholder')}
                ></textarea>
            </div>
            <button type="submit" disabled={saving}>
                {saving ? $_('common.saving') : $_('projectSettings.saveChanges')}
            </button>
        </form>
    </section>

    <section class="card">
        <h3>{$_('projectSettings.inviteCode')}</h3>
        <p>{$_('projectSettings.shareCodeHint')}</p>
        <div class="invite-row">
            <code>{$currentProject?.invite_code}</code>
            <button class="btn-secondary" onclick={handleRegenerateCode}>
                {$_('projectSettings.regenerateCode')}
            </button>
        </div>
    </section>

    <section class="card">
        <h3>{$_('projectSettings.inviteSettings')}</h3>
        <div class="setting-row">
            <label class="toggle-label">
                <input type="checkbox" bind:checked={invitesEnabled} />
                <span class="toggle-text">{$_('projectSettings.allowNewMembers')}</span>
            </label>
        </div>
        <div class="setting-row">
            <label class="toggle-label">
                <input type="checkbox" bind:checked={requireApproval} />
                <span class="toggle-text">{$_('projectSettings.requireApproval')}</span>
            </label>
        </div>
        <button class="btn-secondary" onclick={handleSaveSettings} disabled={savingSettings}>
            {savingSettings ? $_('common.saving') : $_('projectSettings.saveSettings')}
        </button>
    </section>

    {#if requireApproval}
        <section class="card">
            <h3>{$_('projectSettings.pendingRequests')}</h3>
            {#if loadingPending}
                <p class="muted">{$_('common.loading')}</p>
            {:else if pendingMembers.length === 0}
                <p class="muted">{$_('projectSettings.noPendingRequests')}</p>
            {:else}
                <ul class="pending-list">
                    {#each pendingMembers as member}
                        <li class="pending-item">
                            <div class="member-info">
                                <span class="member-name">{member.display_name || member.username}</span>
                                <span class="member-username">@{member.username}</span>
                            </div>
                            <div class="member-actions">
                                <button
                                    class="btn-approve"
                                    onclick={() => handleApproveMember(member.user_id)}
                                    disabled={processingMember === member.user_id}
                                >
                                    {$_('projectSettings.approve')}
                                </button>
                                <button
                                    class="btn-reject"
                                    onclick={() => handleRejectMember(member.user_id)}
                                    disabled={processingMember === member.user_id}
                                >
                                    {$_('projectSettings.reject')}
                                </button>
                            </div>
                        </li>
                    {/each}
                </ul>
            {/if}
        </section>
    {/if}

    <section class="card danger-zone">
        <h3>{$_('projectSettings.dangerZone')}</h3>
        {#if !showDeleteConfirm}
            <p>{$_('projectSettings.deleteWarning')}</p>
            <button class="btn-danger" onclick={() => showDeleteConfirm = true}>
                {$_('projectSettings.deleteProject')}
            </button>
        {:else}
            <p>{$_('projectSettings.typeNameToConfirm')}: <strong>{$currentProject?.name}</strong></p>
            <div class="delete-confirm">
                <input
                    type="text"
                    bind:value={deleteConfirmText}
                    placeholder={$_('projectSettings.projectNamePlaceholder')}
                />
                <button
                    class="btn-danger"
                    onclick={handleDelete}
                    disabled={deleting || deleteConfirmText !== $currentProject?.name}
                >
                    {deleting ? $_('projectSettings.deleting') : $_('projectSettings.confirmDelete')}
                </button>
                <button class="btn-secondary" onclick={() => { showDeleteConfirm = false; deleteConfirmText = ''; }}>
                    {$_('common.cancel')}
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

    /* Invite settings */
    .setting-row {
        margin-bottom: 1rem;
    }

    .toggle-label {
        display: flex;
        align-items: center;
        gap: 0.75rem;
        cursor: pointer;
    }

    .toggle-label input[type="checkbox"] {
        width: 1.2rem;
        height: 1.2rem;
        cursor: pointer;
    }

    .toggle-text {
        font-size: 0.95rem;
    }

    /* Pending members */
    .muted {
        color: #666;
        font-style: italic;
    }

    .pending-list {
        list-style: none;
        padding: 0;
        margin: 0;
    }

    .pending-item {
        display: flex;
        justify-content: space-between;
        align-items: center;
        padding: 1rem;
        background: #f8f9fa;
        border-radius: 8px;
        margin-bottom: 0.75rem;
    }

    .member-info {
        display: flex;
        flex-direction: column;
        gap: 0.25rem;
    }

    .member-name {
        font-weight: 600;
    }

    .member-username {
        font-size: 0.85rem;
        color: #666;
    }

    .member-actions {
        display: flex;
        gap: 0.5rem;
    }

    .btn-approve {
        padding: 0.5rem 1rem;
        background: #28a745;
        color: white;
        border: none;
        border-radius: 6px;
        font-size: 0.9rem;
        cursor: pointer;
    }

    .btn-approve:hover:not(:disabled) {
        background: #218838;
    }

    .btn-reject {
        padding: 0.5rem 1rem;
        background: #dc3545;
        color: white;
        border: none;
        border-radius: 6px;
        font-size: 0.9rem;
        cursor: pointer;
    }

    .btn-reject:hover:not(:disabled) {
        background: #c82333;
    }

    .btn-approve:disabled,
    .btn-reject:disabled {
        opacity: 0.6;
        cursor: not-allowed;
    }

    /* Mobile responsive */
    @media (max-width: 768px) {
        .invite-row {
            flex-direction: column;
            align-items: stretch;
        }

        .invite-row code {
            text-align: center;
        }

        .pending-item {
            flex-direction: column;
            gap: 1rem;
            align-items: stretch;
        }

        .member-actions {
            justify-content: flex-end;
        }

        .delete-confirm {
            flex-direction: column;
        }
    }
</style>
