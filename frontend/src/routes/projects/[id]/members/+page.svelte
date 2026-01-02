<script lang="ts">
	import { page } from '$app/stores';
	import { updateMemberRole, removeMember, setMemberParticipant } from '$lib/api';
	import {
		members,
		participants,
		refreshMembers,
		isAdmin,
		currentProject
	} from '$lib/stores/project';
	import { auth } from '$lib/auth';
	import { _ } from '$lib/i18n';

	let error = $state('');
	let success = $state('');

	let projectId = $derived(parseInt($page.params.id ?? ''));

	// Edit state
	let editingId = $state<number | null>(null);
	let editRole = $state('');
	let editParticipantId = $state<number | null>(null);
	let updating = $state(false);

	function startEdit(m: (typeof $members)[0]) {
		editingId = m.user_id;
		editRole = m.role;
		editParticipantId = m.participant_id;
	}

	function cancelEdit() {
		editingId = null;
		editRole = '';
		editParticipantId = null;
	}

	async function handleUpdateRole(e: Event) {
		e.preventDefault();
		if (!editingId) return;

		updating = true;
		error = '';
		success = '';

		try {
			await updateMemberRole(projectId, editingId, editRole);
			success = 'Role updated successfully';
			cancelEdit();
			await refreshMembers(projectId);
		} catch (e) {
			error = e instanceof Error ? e.message : 'Failed to update role';
		} finally {
			updating = false;
		}
	}

	async function handleSetParticipant(userId: number, participantId: number | null) {
		error = '';
		success = '';

		try {
			await setMemberParticipant(projectId, userId, participantId);
			success = 'Participant link updated';
			await refreshMembers(projectId);
		} catch (e) {
			error = e instanceof Error ? e.message : 'Failed to update participant link';
		}
	}

	async function handleRemove(userId: number) {
		if (!confirm('Remove this member from the project?')) return;

		error = '';
		success = '';

		try {
			await removeMember(projectId, userId);
			success = 'Member removed successfully';
			await refreshMembers(projectId);
		} catch (e) {
			error = e instanceof Error ? e.message : 'Failed to remove member';
		}
	}

	function getRoleBadge(role: string): string {
		switch (role) {
			case 'admin':
				return 'Admin';
			case 'editor':
				return 'Editor';
			case 'reader':
				return 'Reader';
			default:
				return role;
		}
	}

	function isCurrentUser(userId: number): boolean {
		return $auth.user?.id === userId;
	}

	// Get unlinked participants (not associated with any member)
	let unlinkedParticipants = $derived($participants.filter((p) => p.user_id === null));
</script>

<h2>{$_('members.title')}</h2>

{#if error}
	<div class="error">{error}</div>
{/if}

{#if success}
	<div class="success">{success}</div>
{/if}

{#if $isAdmin && $currentProject?.invite_code}
	<section class="card invite-card">
		<h3>{$_('members.inviteLink')}</h3>
		<p>{$_('members.shareCodeHint')}</p>
		<div class="invite-code">
			<code>{$currentProject.invite_code}</code>
		</div>
	</section>
{/if}

<section class="card">
	<h3>{$_('members.projectMembers')}</h3>

	{#if $members.length === 0}
		<p class="empty">{$_('members.noMembersYet')}</p>
	{:else}
		<ul class="members-list">
			{#each $members as m (m.user_id)}
				<li>
					{#if editingId === m.user_id && $isAdmin}
						<form class="edit-form" onsubmit={handleUpdateRole}>
							<div class="member-info">
								<span class="name">{m.display_name || m.username}</span>
								<span class="username">@{m.username}</span>
							</div>
							<div class="edit-controls">
								<select bind:value={editRole}>
									<option value="reader">{$_('roles.reader')}</option>
									<option value="editor">{$_('roles.editor')}</option>
									<option value="admin">{$_('roles.admin')}</option>
								</select>
								<select
									bind:value={editParticipantId}
									onchange={() => handleSetParticipant(m.user_id, editParticipantId)}
								>
									<option value={null}>{$_('members.noParticipant')}</option>
									{#if m.participant_id}
										<option value={m.participant_id}>{m.participant_name}</option>
									{/if}
									{#each unlinkedParticipants as p (p.id)}
										<option value={p.id}>{p.name}</option>
									{/each}
								</select>
								<button type="submit" disabled={updating}>{$_('common.save')}</button>
								<button type="button" onclick={cancelEdit}>{$_('common.cancel')}</button>
							</div>
						</form>
					{:else}
						<div class="member-info">
							<span class="name">
								{m.display_name || m.username}
								{#if isCurrentUser(m.user_id)}
									<span class="you">{$_('members.you')}</span>
								{/if}
							</span>
							<span class="username">@{m.username}</span>
							{#if m.participant_name}
								<span class="participant">{$_('members.as')} {m.participant_name}</span>
							{/if}
						</div>
						<div class="member-actions">
							<span class="role-badge role-{m.role}">{getRoleBadge(m.role)}</span>
							{#if $isAdmin && !isCurrentUser(m.user_id)}
								<button class="btn-edit" onclick={() => startEdit(m)}>{$_('common.edit')}</button>
								<button class="btn-delete" onclick={() => handleRemove(m.user_id)}
									>{$_('members.remove')}</button
								>
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

	.invite-card p {
		color: #666;
		margin-bottom: 1rem;
	}

	.invite-code {
		display: flex;
		align-items: center;
		gap: 1rem;
	}

	.invite-code code {
		background: #f5f5f5;
		padding: 1rem 1.5rem;
		border-radius: 8px;
		font-size: 1.2rem;
		font-weight: 600;
		letter-spacing: 0.1em;
	}

	.empty {
		color: #666;
		font-style: italic;
	}

	.members-list {
		list-style: none;
		padding: 0;
		margin: 0;
	}

	.members-list li {
		display: flex;
		justify-content: space-between;
		align-items: center;
		padding: 1rem;
		border-bottom: 1px solid #eee;
	}

	.members-list li:last-child {
		border-bottom: none;
	}

	.member-info {
		display: flex;
		align-items: center;
		gap: 0.75rem;
	}

	.name {
		font-weight: 600;
	}

	.you {
		color: #666;
		font-weight: 400;
		font-size: 0.9rem;
	}

	.username {
		color: #999;
		font-size: 0.9rem;
	}

	.participant {
		background: #e0e7ff;
		color: #4338ca;
		padding: 0.25rem 0.5rem;
		border-radius: 4px;
		font-size: 0.75rem;
	}

	.member-actions {
		display: flex;
		align-items: center;
		gap: 0.5rem;
	}

	.role-badge {
		font-size: 0.75rem;
		padding: 0.25rem 0.75rem;
		border-radius: 4px;
		font-weight: 600;
	}

	.role-admin {
		background: #7b61ff;
		color: white;
	}

	.role-editor {
		background: #61c4ff;
		color: white;
	}

	.role-reader {
		background: #e0e0e0;
		color: #666;
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
		justify-content: space-between;
		align-items: center;
		width: 100%;
		gap: 1rem;
	}

	.edit-controls {
		display: flex;
		gap: 0.5rem;
		align-items: center;
	}

	.edit-controls select {
		padding: 0.5rem;
		border: 1px solid #ddd;
		border-radius: 6px;
	}

	.edit-controls button {
		padding: 0.5rem 1rem;
		border-radius: 6px;
		font-size: 0.85rem;
		cursor: pointer;
	}

	.edit-controls button[type='submit'] {
		background: var(--accent, #7b61ff);
		color: white;
		border: none;
	}

	.edit-controls button[type='button'] {
		background: transparent;
		border: 1px solid #ddd;
	}

	/* Tablet responsive styles */
	@media (max-width: 768px) {
		.card {
			padding: 1rem;
		}

		.invite-code code {
			font-size: 1rem;
			padding: 0.75rem 1rem;
		}

		.members-list li {
			padding: 0.75rem;
		}

		.member-info {
			gap: 0.5rem;
			flex-wrap: wrap;
		}

		.edit-controls {
			flex-wrap: wrap;
		}

		.edit-controls select {
			min-width: 120px;
		}
	}

	/* Mobile responsive styles */
	@media (max-width: 480px) {
		h2 {
			font-size: 1.3rem;
			margin-bottom: 1rem;
		}

		h3 {
			font-size: 1.1rem;
		}

		.card {
			padding: 0.75rem;
			margin-bottom: 1rem;
		}

		.invite-code {
			flex-direction: column;
			align-items: stretch;
		}

		.invite-code code {
			font-size: 0.9rem;
			padding: 0.6rem 0.8rem;
			text-align: center;
		}

		.members-list li {
			flex-direction: column;
			align-items: flex-start;
			gap: 0.75rem;
			padding: 0.75rem 0.5rem;
		}

		.member-info {
			gap: 0.4rem;
			flex-wrap: wrap;
			width: 100%;
		}

		.name {
			font-size: 0.95rem;
		}

		.username {
			font-size: 0.8rem;
		}

		.participant {
			font-size: 0.7rem;
			padding: 0.2rem 0.4rem;
		}

		.member-actions {
			width: 100%;
			flex-wrap: wrap;
		}

		.role-badge {
			font-size: 0.7rem;
			padding: 0.2rem 0.6rem;
		}

		.btn-edit,
		.btn-delete {
			flex: 1;
			min-width: 80px;
			padding: 0.4rem 0.75rem;
			font-size: 0.8rem;
		}

		.edit-form {
			flex-direction: column;
			align-items: stretch;
			gap: 0.75rem;
		}

		.edit-controls {
			flex-direction: column;
			gap: 0.5rem;
		}

		.edit-controls select,
		.edit-controls button {
			width: 100%;
			font-size: 0.9rem;
		}
	}
</style>
