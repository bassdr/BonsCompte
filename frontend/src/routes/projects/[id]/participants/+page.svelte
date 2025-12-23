<script lang="ts">
    import { page } from '$app/stores';
    import {
        createParticipant,
        updateParticipant,
        deleteParticipant,
        claimParticipant,
        createParticipantInvite,
        getParticipantInvite,
        revokeParticipantInvite,
        type ParticipantInvite
    } from "$lib/api";
    import { participants, refreshParticipants, canEdit, isAdmin, currentProject } from '$lib/stores/project';
    import { auth } from '$lib/auth';

    let error = $state('');
    let success = $state('');

    // Invite state
    let invites = $state<Record<number, ParticipantInvite | null>>({});
    let loadingInvite = $state<number | null>(null);
    let copiedId = $state<number | null>(null);
    let loadingInvites = $state<Set<number>>(new Set()); // Track in-flight requests

    // Form state
    let showAddForm = $state(false);
    let newName = $state('');
    let newWeight = $state('1');
    let newAccountType = $state<'user' | 'pool'>('user');
    let adding = $state(false);

    // Edit state
    let editingId = $state<number | null>(null);
    let editName = $state('');
    let editWeight = $state('');
    let editAccountType = $state<'user' | 'pool'>('user');
    let updating = $state(false);


    let projectId = $derived(parseInt($page.params.id ?? ''));

    async function handleAdd(e: Event) {
        e.preventDefault();
        if (!newName.trim()) return;

        adding = true;
        error = '';
        success = '';

        try {
            const weight = parseFloat(newWeight);
            await createParticipant(projectId, {
                name: newName.trim(),
                default_weight: isNaN(weight) ? 1 : weight,
                account_type: newAccountType
            });

            // Reset form
            newName = '';
            newWeight = '1';
            newAccountType = 'user';
            showAddForm = false;
            success = 'Participant added successfully';

            // Reload participants
            await refreshParticipants(projectId);
        } catch (e) {
            error = e instanceof Error ? e.message : 'Failed to add participant';
        } finally {
            adding = false;
        }
    }

    function startEdit(p: typeof $participants[0]) {
        editingId = p.id;
        editName = p.name;
        editWeight = p.default_weight.toString();
        editAccountType = p.account_type;
    }

    function cancelEdit() {
        editingId = null;
        editName = '';
        editWeight = '';
        editAccountType = 'user';
    }

    async function handleUpdate(e: Event) {
        e.preventDefault();
        if (!editingId || !editName.trim()) return;

        updating = true;
        error = '';
        success = '';

        try {
            const weight = parseFloat(editWeight);
            await updateParticipant(projectId, editingId, {
                name: editName.trim(),
                default_weight: isNaN(weight) ? 1 : weight,
                account_type: editAccountType
            });

            cancelEdit();
            success = 'Participant updated successfully';

            await refreshParticipants(projectId);
        } catch (e) {
            error = e instanceof Error ? e.message : 'Failed to update participant';
        } finally {
            updating = false;
        }
    }

    async function handleDelete(participantId: number) {
        if (!confirm('Delete this participant? This will also remove their contributions.')) return;

        error = '';
        success = '';

        try {
            await deleteParticipant(projectId, participantId);
            success = 'Participant deleted successfully';
            await refreshParticipants(projectId);
        } catch (e) {
            error = e instanceof Error ? e.message : 'Failed to delete participant';
        }
    }

    async function handleClaim(participantId: number) {
        error = '';
        success = '';

        try {
            await claimParticipant(projectId, participantId);
            success = 'You are now associated with this participant';
            await refreshParticipants(projectId);
        } catch (e) {
            error = e instanceof Error ? e.message : 'Failed to claim participant';
        }
    }

    // Check if current user can claim a participant
    function canClaim(p: typeof $participants[0]): boolean {
        if (p.user_id !== null) return false;
        // Check if user already has a participant
        const currentUserId = $auth.user?.id;
        return !$participants.some(part => part.user_id === currentUserId);
    }

    async function handleGenerateInvite(participantId: number) {
        loadingInvite = participantId;
        error = '';

        try {
            const invite = await createParticipantInvite(projectId, participantId);
            invites[participantId] = invite;
            success = 'Invite link generated';
        } catch (e) {
            error = e instanceof Error ? e.message : 'Failed to generate invite';
        } finally {
            loadingInvite = null;
        }
    }

    async function handleRevokeInvite(participantId: number) {
        if (!confirm('Revoke this invite link? Anyone who has it will no longer be able to use it.')) return;

        loadingInvite = participantId;
        error = '';

        try {
            await revokeParticipantInvite(projectId, participantId);
            invites[participantId] = null;
            success = 'Invite link revoked';
        } catch (e) {
            error = e instanceof Error ? e.message : 'Failed to revoke invite';
        } finally {
            loadingInvite = null;
        }
    }

    async function loadInvite(participantId: number) {
        // Prevent duplicate requests
        if (loadingInvites.has(participantId)) return;
        loadingInvites = new Set([...loadingInvites, participantId]);

        try {
            const invite = await getParticipantInvite(projectId, participantId);
            invites[participantId] = invite;
        } catch {
            // No invite exists, that's fine
            invites[participantId] = null;
        } finally {
            loadingInvites = new Set([...loadingInvites].filter(id => id !== participantId));
        }
    }

    function getInviteUrl(invite: ParticipantInvite): string {
        const code = $currentProject?.invite_code || '';
        const baseUrl = typeof window !== 'undefined' ? window.location.origin : '';
        return `${baseUrl}/join?code=${code}&p=${invite.invite_token}`;
    }

    async function copyInviteUrl(participantId: number) {
        const invite = invites[participantId];
        if (!invite) return;

        try {
            await navigator.clipboard.writeText(getInviteUrl(invite));
            copiedId = participantId;
            setTimeout(() => {
                if (copiedId === participantId) copiedId = null;
            }, 2000);
        } catch {
            error = 'Failed to copy to clipboard';
        }
    }

    // Load invites for participants without users (admin only)
    $effect(() => {
        if ($isAdmin && $participants.length > 0) {
            for (const p of $participants) {
                // Only load if not already loaded and not currently loading
                if (p.user_id === null && invites[p.id] === undefined && !loadingInvites.has(p.id)) {
                    loadInvite(p.id);
                }
            }
        }
    });
</script>

<h2>Participants</h2>

{#if error}
    <div class="error">{error}</div>
{/if}

{#if success}
    <div class="success">{success}</div>
{/if}

{#if $canEdit}
    <div class="actions">
        <button class="btn-primary" onclick={() => showAddForm = !showAddForm}>
            {showAddForm ? 'Cancel' : '+ Add Participant'}
        </button>
    </div>
{/if}

{#if showAddForm}
    <div class="card form-card">
        <h3>Add Participant</h3>
        <form onsubmit={handleAdd}>
            <div class="form-row">
                <div class="field">
                    <label for="name">Name</label>
                    <input
                        id="name"
                        type="text"
                        bind:value={newName}
                        placeholder="Participant name"
                        required
                    />
                </div>
                <div class="field small">
                    <label for="weight">Default Weight</label>
                    <input
                        id="weight"
                        type="number"
                        bind:value={newWeight}
                        min="0"
                        step="0.5"
                    />
                </div>
                <div class="field small">
                    <label for="accountType">Type</label>
                    <select id="accountType" bind:value={newAccountType}>
                        <option value="user">User</option>
                        <option value="pool">Pool</option>
                    </select>
                </div>
            </div>
            <button type="submit" disabled={adding}>
                {adding ? 'Adding...' : 'Add Participant'}
            </button>
        </form>
    </div>
{/if}

<section class="card">
    <h3>All Participants</h3>

    {#if $participants.length === 0}
        <p class="empty">No participants yet. Add one to get started.</p>
    {:else}
        <ul class="participants-list">
            {#each $participants as p}
                <li>
                    {#if editingId === p.id}
                        <form class="edit-form" onsubmit={handleUpdate}>
                            <input
                                type="text"
                                bind:value={editName}
                                required
                            />
                            <input
                                type="number"
                                bind:value={editWeight}
                                min="0"
                                step="0.5"
                                class="small-input"
                            />
                            <select bind:value={editAccountType} class="small-select">
                                <option value="user">User</option>
                                <option value="pool">Pool</option>
                            </select>
                            <button type="submit" disabled={updating}>Save</button>
                            <button type="button" onclick={cancelEdit}>Cancel</button>
                        </form>
                    {:else}
                        <div class="participant-row">
                            <div class="participant-info">
                                <span class="name">{p.name}</span>
                                {#if p.account_type === 'pool'}
                                    <span class="pool-badge">Pool</span>
                                {/if}
                                <span class="weight">
                                    {#if p.default_weight === 0}
                                        Weight: 0 (will not participate)
                                    {:else}
                                        Weight: {p.default_weight}
                                    {/if}
                                </span>
                                {#if p.user_id}
                                    <span class="linked">Linked to account</span>
                                {/if}
                            </div>
                            <div class="participant-actions">
                                {#if canClaim(p)}
                                    <button class="btn-claim" onclick={() => handleClaim(p.id)}>
                                        Claim as me
                                    </button>
                                {/if}
                                {#if $canEdit}
                                    <button class="btn-edit" onclick={() => startEdit(p)}>Edit</button>
                                {/if}
                                {#if $isAdmin}
                                    <button class="btn-delete" onclick={() => handleDelete(p.id)}>Delete</button>
                                {/if}
                            </div>
                        </div>

                        {#if $isAdmin && p.user_id === null && p.account_type !== 'pool'}
                            <div class="invite-section">
                                {#if invites[p.id]}
                                    <div class="invite-link-row">
                                        <span class="invite-label">Invite link:</span>
                                        <code class="invite-url">{getInviteUrl(invites[p.id]!)}</code>
                                        <button
                                            class="btn-copy"
                                            onclick={() => copyInviteUrl(p.id)}
                                            disabled={loadingInvite === p.id}
                                        >
                                            {copiedId === p.id ? 'Copied!' : 'Copy'}
                                        </button>
                                        <button
                                            class="btn-revoke"
                                            onclick={() => handleRevokeInvite(p.id)}
                                            disabled={loadingInvite === p.id}
                                        >
                                            Revoke
                                        </button>
                                    </div>
                                    {#if invites[p.id]?.used_by}
                                        <span class="invite-used">Invite already used</span>
                                    {/if}
                                {:else}
                                    <button
                                        class="btn-generate-invite"
                                        onclick={() => handleGenerateInvite(p.id)}
                                        disabled={loadingInvite === p.id}
                                    >
                                        {loadingInvite === p.id ? 'Generating...' : 'Generate Invite Link'}
                                    </button>
                                {/if}
                            </div>
                        {/if}
                    {/if}
                </li>
            {/each}
        </ul>
    {/if}
</section>

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

    .actions {
        margin-bottom: 1.5rem;
    }

    .btn-primary {
        padding: 0.75rem 1.5rem;
        background: var(--accent, #7b61ff);
        color: white;
        border: none;
        border-radius: 8px;
        font-size: 1rem;
        cursor: pointer;
    }

    .form-row {
        display: flex;
        gap: 1rem;
    }

    .field {
        flex: 1;
        margin-bottom: 1rem;
    }

    .field.small {
        flex: 0 0 120px;
    }

    label {
        display: block;
        margin-bottom: 0.5rem;
        font-weight: 600;
        font-size: 0.9rem;
    }

    input {
        width: 100%;
        padding: 0.75rem;
        border: 1px solid #ddd;
        border-radius: 8px;
        font-size: 1rem;
    }

    input:focus {
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

    .empty {
        color: #666;
        font-style: italic;
    }

    .participants-list {
        list-style: none;
        padding: 0;
        margin: 0;
    }

    .participants-list li {
        display: flex;
        flex-direction: column;
        padding: 1rem;
        border-bottom: 1px solid #eee;
    }

    .participants-list li:last-child {
        border-bottom: none;
    }

    .participant-info {
        display: flex;
        align-items: center;
        gap: 1rem;
    }

    .name {
        font-weight: 600;
    }

    .weight {
        color: #666;
        font-size: 0.9rem;
    }

    .linked {
        background: #d4edda;
        color: #155724;
        padding: 0.25rem 0.5rem;
        border-radius: 4px;
        font-size: 0.75rem;
    }

    .pool-badge {
        background: var(--accent, #7b61ff);
        color: white;
        padding: 0.2rem 0.5rem;
        border-radius: 4px;
        font-size: 0.7rem;
        font-weight: 600;
        text-transform: uppercase;
    }

    select {
        padding: 0.75rem;
        border: 1px solid #ddd;
        border-radius: 8px;
        font-size: 1rem;
        background: white;
    }

    select:focus {
        outline: none;
        border-color: var(--accent, #7b61ff);
    }

    .small-select {
        width: 80px;
        padding: 0.5rem;
    }

    .participant-actions {
        display: flex;
        gap: 0.5rem;
    }

    .btn-claim {
        padding: 0.5rem 1rem;
        background: #61c4ff;
        color: white;
        border: none;
        border-radius: 6px;
        font-size: 0.85rem;
        cursor: pointer;
    }

    .btn-edit {
        padding: 0.5rem 1rem;
        background: transparent;
        border: 1px solid #ddd;
        border-radius: 6px;
        font-size: 0.85rem;
        cursor: pointer;
    }

    .btn-edit:hover {
        background: #f5f5f5;
    }

    .btn-delete {
        padding: 0.5rem 1rem;
        background: transparent;
        border: 1px solid #ddd;
        border-radius: 6px;
        font-size: 0.85rem;
        color: #999;
        cursor: pointer;
    }

    .btn-delete:hover {
        background: #fee;
        border-color: #fcc;
        color: #c00;
    }

    .edit-form {
        display: flex;
        gap: 0.5rem;
        align-items: center;
        width: 100%;
    }

    .edit-form input {
        padding: 0.5rem;
    }

    .edit-form .small-input {
        width: 80px;
    }

    .edit-form button {
        padding: 0.5rem 1rem;
        border-radius: 6px;
        font-size: 0.85rem;
        cursor: pointer;
    }

    .edit-form button[type="submit"] {
        background: var(--accent, #7b61ff);
        color: white;
        border: none;
    }

    .edit-form button[type="button"] {
        background: transparent;
        border: 1px solid #ddd;
    }

    /* Participant row layout */
    .participant-row {
        display: flex;
        justify-content: space-between;
        align-items: center;
        width: 100%;
    }

    /* Invite section */
    .invite-section {
        margin-top: 0.75rem;
        padding-top: 0.75rem;
        border-top: 1px dashed #ddd;
        width: 100%;
    }

    .invite-link-row {
        display: flex;
        align-items: center;
        gap: 0.5rem;
        flex-wrap: wrap;
    }

    .invite-label {
        font-size: 0.85rem;
        color: #666;
    }

    .invite-url {
        background: #f5f5f5;
        padding: 0.4rem 0.75rem;
        border-radius: 4px;
        font-size: 0.8rem;
        max-width: 300px;
        overflow: hidden;
        text-overflow: ellipsis;
        white-space: nowrap;
    }

    .btn-copy {
        padding: 0.4rem 0.75rem;
        background: var(--accent, #7b61ff);
        color: white;
        border: none;
        border-radius: 4px;
        font-size: 0.8rem;
        cursor: pointer;
    }

    .btn-copy:hover:not(:disabled) {
        opacity: 0.9;
    }

    .btn-revoke {
        padding: 0.4rem 0.75rem;
        background: transparent;
        color: #c00;
        border: 1px solid #c00;
        border-radius: 4px;
        font-size: 0.8rem;
        cursor: pointer;
    }

    .btn-revoke:hover:not(:disabled) {
        background: #fee;
    }

    .btn-generate-invite {
        padding: 0.5rem 1rem;
        background: #e8e4ff;
        color: var(--accent, #7b61ff);
        border: 1px solid var(--accent, #7b61ff);
        border-radius: 6px;
        font-size: 0.85rem;
        cursor: pointer;
    }

    .btn-generate-invite:hover:not(:disabled) {
        background: #d8d0ff;
    }

    .btn-generate-invite:disabled,
    .btn-copy:disabled,
    .btn-revoke:disabled {
        opacity: 0.6;
        cursor: not-allowed;
    }

    .invite-used {
        font-size: 0.8rem;
        color: #666;
        font-style: italic;
        margin-top: 0.25rem;
        display: block;
    }

    /* Mobile responsive */
    @media (max-width: 768px) {
        .participants-list li {
            flex-direction: column;
            align-items: stretch;
        }

        .participant-row {
            flex-direction: column;
            align-items: flex-start;
            gap: 0.75rem;
        }

        .participant-info {
            flex-direction: column;
            align-items: flex-start;
            gap: 0.25rem;
        }

        .participant-actions {
            flex-wrap: wrap;
        }

        .invite-link-row {
            flex-direction: column;
            align-items: flex-start;
        }

        .invite-url {
            max-width: 100%;
        }
    }
</style>
