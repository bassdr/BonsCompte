<script lang="ts">
    import { page } from '$app/stores';
    import { createParticipant, updateParticipant, deleteParticipant, claimParticipant } from "$lib/api";
    import { participants, refreshParticipants, canEdit, isAdmin } from '$lib/stores/project';
    import { auth } from '$lib/auth';

    let error = $state('');
    let success = $state('');

    // Form state
    let showAddForm = $state(false);
    let newName = $state('');
    let newWeight = $state('1');
    let adding = $state(false);

    // Edit state
    let editingId = $state<number | null>(null);
    let editName = $state('');
    let editWeight = $state('');
    let updating = $state(false);

    let projectId = $derived(parseInt($page.params.id ?? ''));

    async function handleAdd(e: Event) {
        e.preventDefault();
        if (!newName.trim()) return;

        adding = true;
        error = '';
        success = '';

        try {
            await createParticipant(projectId, {
                name: newName.trim(),
                default_weight: parseFloat(newWeight) || 1
            });

            // Reset form
            newName = '';
            newWeight = '1';
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
    }

    function cancelEdit() {
        editingId = null;
        editName = '';
        editWeight = '';
    }

    async function handleUpdate(e: Event) {
        e.preventDefault();
        if (!editingId || !editName.trim()) return;

        updating = true;
        error = '';
        success = '';

        try {
            await updateParticipant(projectId, editingId, {
                name: editName.trim(),
                default_weight: parseFloat(editWeight) || 1
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
                            <button type="submit" disabled={updating}>Save</button>
                            <button type="button" onclick={cancelEdit}>Cancel</button>
                        </form>
                    {:else}
                        <div class="participant-info">
                            <span class="name">{p.name}</span>
                            <span class="weight">Weight: {p.default_weight}</span>
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
        justify-content: space-between;
        align-items: center;
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
</style>
