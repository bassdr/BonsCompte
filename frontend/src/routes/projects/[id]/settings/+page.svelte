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
		createParticipant,
		updateParticipant,
		deleteParticipant,
		claimParticipant,
		createParticipantInvite,
		getParticipantInvite,
		revokeParticipantInvite,
		updateMemberRole,
		removeMember,
		setMemberParticipant,
		type ProjectMember,
		type ParticipantInvite
	} from '$lib/api';
	import {
		currentProject,
		loadProject,
		isAdmin,
		participants,
		refreshParticipants,
		canEdit,
		members,
		refreshMembers
	} from '$lib/stores/project';
	import { auth } from '$lib/auth';
	import { _ } from '$lib/i18n';

	let error = $state('');
	let success = $state('');

	let projectId = $derived(parseInt($page.params.id ?? ''));

	// Project Details
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

	// Participants
	let participantInvites = $state<Record<number, ParticipantInvite | null>>({});
	let loadingInvite = $state<number | null>(null);
	let copiedId = $state<number | null>(null);
	let loadingInvites = $state<Set<number>>(new Set());

	let showAddParticipantForm = $state(false);
	let newParticipantName = $state('');
	let newParticipantWeight = $state('1');
	let newParticipantAccountType = $state<'user' | 'pool'>('user');
	let addingParticipant = $state(false);

	let editingParticipantId = $state<number | null>(null);
	let editParticipantName = $state('');
	let editParticipantWeight = $state('');
	let editParticipantAccountType = $state<'user' | 'pool'>('user');
	let updatingParticipant = $state(false);

	// Members
	let editingMemberId = $state<number | null>(null);
	let editMemberRole = $state('');
	let editMemberParticipantId = $state<number | null>(null);
	let updatingMember = $state(false);

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

	// Load pending members
	$effect(() => {
		if (projectId && $isAdmin) {
			loadPendingMembers();
		}
	});

	// Load participant invites
	$effect(() => {
		if ($isAdmin && $participants.length > 0) {
			for (const p of $participants) {
				if (
					p.user_id === null &&
					participantInvites[p.id] === undefined &&
					!loadingInvites.has(p.id)
				) {
					loadParticipantInvite(p.id);
				}
			}
		}
	});

	let unlinkedParticipants = $derived($participants.filter((p) => p.user_id === null));

	// Project Details
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

			success = $_('projectSettings.handleSave.success');
			await loadProject(projectId);
		} catch (e) {
			error = e instanceof Error ? e.message : $_('projectSettings.handleSave.fail');
		} finally {
			saving = false;
		}
	}

	async function handleRegenerateCode() {
		if (!confirm($_('projectSettings.handleRegenerateCode.confirm'))) return;

		error = '';
		success = '';

		try {
			await regenerateInviteCode(projectId);
			success = $_('projectSettings.handleRegenerateCode.success');
			await loadProject(projectId);
		} catch (e) {
			error = e instanceof Error ? e.message : $_('projectSettings.handleRegenerateCode.fail');
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
			success = $_('projectSettings.handleSaveSettings.success');
			await loadProject(projectId);
		} catch (e) {
			error = e instanceof Error ? e.message : $_('projectSettings.handleSaveSettings.fail');
		} finally {
			savingSettings = false;
		}
	}

	async function loadPendingMembers() {
		loadingPending = true;
		try {
			pendingMembers = await getPendingMembers(projectId);
		} catch (e) {
			console.error($_('projectSettings.loadPendingMembers.fail'), e);
		} finally {
			loadingPending = false;
		}
	}

	async function handleApproveMember(userId: number) {
		processingMember = userId;
		error = '';

		try {
			await approveMember(projectId, userId);
			await loadPendingMembers();
			success = $_('projectSettings.handleApproveMember.success');
		} catch (e) {
			error = e instanceof Error ? e.message : $_('projectSettings.handleApproveMember.fail');
		} finally {
			processingMember = null;
		}
	}

	async function handleRejectMember(userId: number) {
		if (!confirm($_('projectSettings.handleRejectMember.confirm'))) return;

		processingMember = userId;
		error = '';

		try {
			await rejectMember(projectId, userId);
			await loadPendingMembers();
			success = $_('projectSettings.handleRejectMember.success');
		} catch (e) {
			error = e instanceof Error ? e.message : $_('projectSettings.handleRejectMember.fail');
		} finally {
			processingMember = null;
		}
	}

	// Participants
	async function handleAddParticipant(e: Event) {
		e.preventDefault();
		if (!newParticipantName.trim()) return;

		addingParticipant = true;
		error = '';
		success = '';

		try {
			const weight = parseFloat(newParticipantWeight);
			await createParticipant(projectId, {
				name: newParticipantName.trim(),
				default_weight: isNaN(weight) ? 1 : weight,
				account_type: newParticipantAccountType
			});

			newParticipantName = '';
			newParticipantWeight = '1';
			newParticipantAccountType = 'user';
			showAddParticipantForm = false;
			success = $_('projectSettings.handleAddParticipant.success');

			await refreshParticipants(projectId);
		} catch (e) {
			error = e instanceof Error ? e.message : $_('projectSettings.handleAddParticipant.fail');
		} finally {
			addingParticipant = false;
		}
	}

	function startEditParticipant(p: (typeof $participants)[0]) {
		editingParticipantId = p.id;
		editParticipantName = p.name;
		editParticipantWeight = p.default_weight.toString();
		editParticipantAccountType = p.account_type;
	}

	function cancelEditParticipant() {
		editingParticipantId = null;
		editParticipantName = '';
		editParticipantWeight = '';
		editParticipantAccountType = 'user';
	}

	async function handleUpdateParticipant(e: Event) {
		e.preventDefault();
		if (!editingParticipantId || !editParticipantName.trim()) return;

		updatingParticipant = true;
		error = '';
		success = '';

		try {
			const weight = parseFloat(editParticipantWeight);
			await updateParticipant(projectId, editingParticipantId, {
				name: editParticipantName.trim(),
				default_weight: isNaN(weight) ? 1 : weight,
				account_type: editParticipantAccountType
			});

			cancelEditParticipant();
			success = $_('projectSettings.handleUpdateParticipant.success');

			await refreshParticipants(projectId);
		} catch (e) {
			error = e instanceof Error ? e.message : $_('projectSettings.handleUpdateParticipant.fail');
		} finally {
			updatingParticipant = false;
		}
	}

	async function handleDeleteParticipant(participantId: number) {
		if (!confirm($_('projectSettings.handleDeleteParticipant.confirm'))) return;

		error = '';
		success = '';

		try {
			await deleteParticipant(projectId, participantId);
			success = $_('projectSettings.handleDeleteParticipant.success');
			await refreshParticipants(projectId);
		} catch (e) {
			error = e instanceof Error ? e.message : $_('projectSettings.handleDeleteParticipant.fail');
		}
	}

	async function handleClaimParticipant(participantId: number) {
		error = '';
		success = '';

		try {
			await claimParticipant(projectId, participantId);
			success = $_('projectSettings.handleClaimParticipant.success');
			await refreshParticipants(projectId);
		} catch (e) {
			error = e instanceof Error ? e.message : $_('projectSettings.handleClaimParticipant.fail');
		}
	}

	function canClaimParticipant(p: (typeof $participants)[0]): boolean {
		if (p.user_id !== null) return false;
		const currentUserId = $auth.user?.id;
		return !$participants.some((part) => part.user_id === currentUserId);
	}

	async function handleGenerateParticipantInvite(participantId: number) {
		loadingInvite = participantId;
		error = '';

		try {
			const invite = await createParticipantInvite(projectId, participantId);
			participantInvites[participantId] = invite;
			success = $_('projectSettings.handleGenerateParticipantInvite.success');
		} catch (e) {
			if (e instanceof Error) error = e.message;
			else error = $_('projectSettings.handleGenerateParticipantInvite.fail');
		} finally {
			loadingInvite = null;
		}
	}

	async function handleRevokeParticipantInvite(participantId: number) {
		if (!confirm($_('projectSettings.handleRevokeParticipantInvite.confirm'))) return;

		loadingInvite = participantId;
		error = '';

		try {
			await revokeParticipantInvite(projectId, participantId);
			participantInvites[participantId] = null;
			success = $_('projectSettings.handleRevokeParticipantInvite.success');
		} catch (e) {
			if (e instanceof Error) error = e.message;
			else error = $_('projectSettings.handleRevokeParticipantInvite.fail');
		} finally {
			loadingInvite = null;
		}
	}

	async function loadParticipantInvite(participantId: number) {
		if (loadingInvites.has(participantId)) return;
		loadingInvites = new Set([...loadingInvites, participantId]);

		try {
			const invite = await getParticipantInvite(projectId, participantId);
			participantInvites[participantId] = invite;
		} catch {
			participantInvites[participantId] = null;
		} finally {
			loadingInvites = new Set([...loadingInvites].filter((id) => id !== participantId));
		}
	}

	function getParticipantInviteUrl(invite: ParticipantInvite): string {
		const code = $currentProject?.invite_code || '';
		const baseUrl = typeof window !== 'undefined' ? window.location.origin : '';
		return `${baseUrl}/join?code=${code}&p=${invite.invite_token}`;
	}

	async function copyParticipantInviteUrl(participantId: number) {
		const invite = participantInvites[participantId];
		if (!invite) return;

		try {
			await navigator.clipboard.writeText(getParticipantInviteUrl(invite));
			copiedId = participantId;
			setTimeout(() => {
				if (copiedId === participantId) copiedId = null;
			}, 2000);
		} catch {
			error = $_('projectSettings.copyParticipantInviteUrl.fail');
		}
	}

	// Members
	function startEditMember(m: (typeof $members)[0]) {
		editingMemberId = m.user_id;
		editMemberRole = m.role;
		editMemberParticipantId = m.participant_id;
	}

	function cancelEditMember() {
		editingMemberId = null;
		editMemberRole = '';
		editMemberParticipantId = null;
	}

	async function handleUpdateMemberRole(e: Event) {
		e.preventDefault();
		if (!editingMemberId) return;

		updatingMember = true;
		error = '';
		success = '';

		try {
			await updateMemberRole(projectId, editingMemberId, editMemberRole);
			success = $_('projectSettings.handleUpdateMemberRole.success');
			cancelEditMember();
			await refreshMembers(projectId);
		} catch (e) {
			error = e instanceof Error ? e.message : $_('projectSettings.handleUpdateMemberRole.fail');
		} finally {
			updatingMember = false;
		}
	}

	async function handleSetMemberParticipant(userId: number, participantId: number | null) {
		error = '';
		success = '';

		try {
			await setMemberParticipant(projectId, userId, participantId);
			success = $_('projectSettings.handleSetMemberParticipant.success');
			await refreshMembers(projectId);
		} catch (e) {
			if (e instanceof Error) error = e.message;
			else error = $_('projectSettings.handleSetMemberParticipant.fail');
		}
	}

	async function handleRemoveMember(userId: number) {
		if (!confirm($_('projectSettings.handleRemoveMember.confirm'))) return;

		error = '';
		success = '';

		try {
			await removeMember(projectId, userId);
			success = $_('projectSettings.handleRemoveMember.success');
			await refreshMembers(projectId);
		} catch (e) {
			error = e instanceof Error ? e.message : $_('projectSettings.handleRemoveMember.fail');
		}
	}

	function isCurrentUser(userId: number): boolean {
		return $auth.user?.id === userId;
	}

	// Delete Project
	async function handleDelete() {
		if (deleteConfirmText !== $currentProject?.name) {
			error = $_('projectSettings.handleDelete.cancel');
			return;
		}

		deleting = true;
		error = '';

		try {
			await deleteProject(projectId);
			await goto('/');
		} catch (e) {
			error = e instanceof Error ? e.message : $_('projectSettings.handleDelete.fail');
			deleting = false;
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

	<!-- Project Details -->
	<section class="card">
		<h3>{$_('projectSettings.projectDetails')}</h3>
		<form onsubmit={handleSave}>
			<div class="field">
				<label for="name">{$_('projectSettings.projectName')}</label>
				<input id="name" type="text" bind:value={name} required />
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

	<!-- Invite Code -->
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

	<!-- Invite Settings -->
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

	<!-- Pending Requests -->
	{#if requireApproval}
		<section class="card">
			<h3>{$_('projectSettings.pendingRequests')}</h3>
			{#if loadingPending}
				<p class="muted">{$_('common.loading')}</p>
			{:else if pendingMembers.length === 0}
				<p class="muted">{$_('projectSettings.noPendingRequests')}</p>
			{:else}
				<ul class="pending-list">
					{#each pendingMembers as member (member.user_id)}
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

	<!-- Participants Section -->
	<section class="card">
		<h3>{$_('participants.title')}</h3>
		<p class="section-desc">{$_('participants.description')}</p>

		{#if $canEdit}
			<div class="actions">
				<button
					class="btn-primary"
					onclick={() => (showAddParticipantForm = !showAddParticipantForm)}
				>
					{showAddParticipantForm ? $_('common.cancel') : $_('participants.addParticipant')}
				</button>
			</div>
		{/if}

		{#if showAddParticipantForm}
			<div class="form-card">
				<form onsubmit={handleAddParticipant}>
					<div class="form-row">
						<div class="field">
							<label for="participant-name">{$_('participants.name')}</label>
							<input
								id="participant-name"
								type="text"
								bind:value={newParticipantName}
								placeholder={$_('participants.participantNamePlaceholder')}
								required
							/>
						</div>
						<div class="field small">
							<label for="participant-weight">{$_('participants.defaultWeight')}</label>
							<input
								id="participant-weight"
								type="number"
								bind:value={newParticipantWeight}
								min="0"
								step="0.5"
							/>
						</div>
						<div class="field small">
							<label for="participant-type">{$_('participants.type')}</label>
							<select id="participant-type" bind:value={newParticipantAccountType}>
								<option value="user">{$_('participants.user')}</option>
								<option value="pool">{$_('participants.pool')}</option>
							</select>
						</div>
					</div>
					<button type="submit" disabled={addingParticipant}>
						{addingParticipant ? $_('participants.adding') : $_('participants.addParticipant')}
					</button>
				</form>
			</div>
		{/if}

		{#if $participants.length === 0}
			<p class="empty">{$_('participants.noParticipantsYet')}</p>
		{:else}
			<ul class="list">
				{#each $participants as p (p.id)}
					<li>
						{#if editingParticipantId === p.id}
							<form class="edit-form" onsubmit={handleUpdateParticipant}>
								<input type="text" bind:value={editParticipantName} required />
								<input
									type="number"
									bind:value={editParticipantWeight}
									min="0"
									step="0.5"
									class="small-input"
								/>
								<select bind:value={editParticipantAccountType} class="small-select">
									<option value="user">{$_('participants.user')}</option>
									<option value="pool">{$_('participants.pool')}</option>
								</select>
								<button type="submit" disabled={updatingParticipant}>{$_('common.save')}</button>
								<button type="button" onclick={cancelEditParticipant}>{$_('common.cancel')}</button>
							</form>
						{:else}
							<div class="item-row">
								<div class="item-info">
									<span class="name">{p.name}</span>
									{#if p.account_type === 'pool'}
										<span class="pool-badge">{$_('participants.pool')}</span>
									{/if}
									<span class="weight">
										{#if p.default_weight === 0}
											{$_('participants.weightZero')}
										{:else}
											{$_('participants.weight')}: {p.default_weight}
										{/if}
									</span>
									{#if p.user_id}
										<span class="linked">{$_('participants.linkedToAccount')}</span>
									{/if}
								</div>
								<div class="item-actions">
									{#if canClaimParticipant(p)}
										<button class="btn-claim" onclick={() => handleClaimParticipant(p.id)}>
											{$_('participants.claimAsMe')}
										</button>
									{/if}
									{#if $canEdit}
										<button class="btn-edit" onclick={() => startEditParticipant(p)}
											>{$_('common.edit')}</button
										>
									{/if}
									{#if $isAdmin}
										<button class="btn-delete" onclick={() => handleDeleteParticipant(p.id)}
											>{$_('common.delete')}</button
										>
									{/if}
								</div>
							</div>

							{#if $isAdmin && p.user_id === null && p.account_type !== 'pool'}
								<div class="invite-section">
									{#if participantInvites[p.id]}
										<div class="invite-link-row">
											<span class="invite-label">{$_('participants.inviteLink')}:</span>
											<code class="invite-url"
												>{getParticipantInviteUrl(participantInvites[p.id]!)}</code
											>
											<button
												class="btn-copy"
												onclick={() => copyParticipantInviteUrl(p.id)}
												disabled={loadingInvite === p.id}
											>
												{copiedId === p.id ? $_('participants.copied') : $_('participants.copy')}
											</button>
											<button
												class="btn-revoke"
												onclick={() => handleRevokeParticipantInvite(p.id)}
												disabled={loadingInvite === p.id}
											>
												{$_('participants.revoke')}
											</button>
										</div>
										{#if participantInvites[p.id]?.used_by}
											<span class="invite-used">{$_('participants.inviteAlreadyUsed')}</span>
										{/if}
									{:else}
										<button
											class="btn-generate-invite"
											onclick={() => handleGenerateParticipantInvite(p.id)}
											disabled={loadingInvite === p.id}
										>
											{loadingInvite === p.id
												? $_('participants.generating')
												: $_('participants.generateInviteLink')}
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

	<!-- Members Section -->
	<section class="card">
		<h3>{$_('members.title')}</h3>
		<p class="section-desc">{$_('members.description')}</p>

		{#if $members.length === 0}
			<p class="empty">{$_('members.noMembersYet')}</p>
		{:else}
			<ul class="list">
				{#each $members as m (m.user_id)}
					<li>
						{#if editingMemberId === m.user_id}
							<form class="edit-form" onsubmit={handleUpdateMemberRole}>
								<div class="member-info">
									<span class="name">{m.display_name || m.username}</span>
									<span class="username">@{m.username}</span>
								</div>
								<div class="edit-controls">
									<select bind:value={editMemberRole}>
										<option value="reader">{$_('roles.reader')}</option>
										<option value="editor">{$_('roles.editor')}</option>
										<option value="admin">{$_('roles.admin')}</option>
									</select>
									<select
										bind:value={editMemberParticipantId}
										onchange={() => handleSetMemberParticipant(m.user_id, editMemberParticipantId)}
									>
										<option value={null}>{$_('members.noParticipant')}</option>
										{#if m.participant_id}
											<option value={m.participant_id}>{m.participant_name}</option>
										{/if}
										{#each unlinkedParticipants as p (p.id)}
											<option value={p.id}>{p.name}</option>
										{/each}
									</select>
									<button type="submit" disabled={updatingMember}>{$_('common.save')}</button>
									<button type="button" onclick={cancelEditMember}>{$_('common.cancel')}</button>
								</div>
							</form>
						{:else}
							<div class="item-row">
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
								<div class="item-actions">
									<span class="role-badge role-{m.role}">{$_(`roles.${m.role}`)}</span>
									{#if !isCurrentUser(m.user_id)}
										<button class="btn-edit" onclick={() => startEditMember(m)}
											>{$_('common.edit')}</button
										>
										<button class="btn-delete" onclick={() => handleRemoveMember(m.user_id)}
											>{$_('members.remove')}</button
										>
									{/if}
								</div>
							</div>
						{/if}
					</li>
				{/each}
			</ul>
		{/if}
	</section>

	<!-- Danger Zone -->
	<section class="card danger-zone">
		<h3>{$_('projectSettings.dangerZone')}</h3>
		{#if !showDeleteConfirm}
			<p>{$_('projectSettings.deleteWarning')}</p>
			<button class="btn-danger" onclick={() => (showDeleteConfirm = true)}>
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
				<button
					class="btn-secondary"
					onclick={() => {
						showDeleteConfirm = false;
						deleteConfirmText = '';
					}}
				>
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

	.section-desc {
		color: #666;
		font-size: 0.9rem;
		margin-bottom: 1rem;
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

	.field.small {
		flex: 0 0 120px;
	}

	label {
		display: block;
		margin-bottom: 0.5rem;
		font-weight: 600;
		font-size: 0.9rem;
	}

	input,
	textarea,
	select {
		width: 100%;
		padding: 0.75rem;
		border: 1px solid #ddd;
		border-radius: 8px;
		font-size: 1rem;
		font-family: inherit;
	}

	input:focus,
	textarea:focus,
	select:focus {
		outline: none;
		border-color: var(--accent, #7b61ff);
	}

	button[type='submit'] {
		padding: 0.75rem 1.5rem;
		background: var(--accent, #7b61ff);
		color: white;
		border: none;
		border-radius: 8px;
		font-size: 1rem;
		cursor: pointer;
	}

	button[type='submit']:disabled {
		opacity: 0.6;
		cursor: not-allowed;
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

	.setting-row {
		margin-bottom: 1rem;
	}

	.toggle-label {
		display: flex;
		align-items: center;
		gap: 0.75rem;
		cursor: pointer;
	}

	.toggle-label input[type='checkbox'] {
		width: 1.2rem;
		height: 1.2rem;
		cursor: pointer;
	}

	.toggle-text {
		font-size: 0.95rem;
	}

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
		align-items: center;
		gap: 0.75rem;
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

	.actions {
		margin-bottom: 1.5rem;
	}

	.form-card {
		background: #f8f9fa;
		padding: 1rem;
		border-radius: 8px;
		margin-bottom: 1.5rem;
	}

	.form-row {
		display: flex;
		gap: 1rem;
	}

	.empty {
		color: #666;
		font-style: italic;
	}

	.list {
		list-style: none;
		padding: 0;
		margin: 0;
	}

	.list li {
		display: flex;
		flex-direction: column;
		padding: 1rem;
		border-bottom: 1px solid #eee;
	}

	.list li:last-child {
		border-bottom: none;
	}

	.item-row {
		display: flex;
		justify-content: space-between;
		align-items: center;
		width: 100%;
	}

	.item-info {
		display: flex;
		align-items: center;
		gap: 1rem;
	}

	.item-actions {
		display: flex;
		gap: 0.5rem;
	}

	.name {
		font-weight: 600;
	}

	.username {
		color: #999;
		font-size: 0.9rem;
	}

	.you {
		color: #666;
		font-weight: 400;
		font-size: 0.9rem;
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

	.participant {
		background: #e0e7ff;
		color: #4338ca;
		padding: 0.25rem 0.5rem;
		border-radius: 4px;
		font-size: 0.75rem;
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

	.edit-form .small-select {
		width: 80px;
		padding: 0.5rem;
	}

	.edit-form button {
		padding: 0.5rem 1rem;
		border-radius: 6px;
		font-size: 0.85rem;
		cursor: pointer;
	}

	.edit-form button[type='submit'] {
		background: var(--accent, #7b61ff);
		color: white;
		border: none;
	}

	.edit-form button[type='button'] {
		background: transparent;
		border: 1px solid #ddd;
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

	@media (max-width: 768px) {
		.invite-row {
			flex-direction: column;
			align-items: stretch;
		}

		.invite-row code {
			text-align: center;
		}

		.pending-item,
		.item-row {
			flex-direction: column;
			gap: 0.75rem;
			align-items: flex-start;
		}

		.member-actions,
		.item-actions {
			flex-wrap: wrap;
		}

		.delete-confirm {
			flex-direction: column;
		}

		.form-row {
			flex-direction: column;
		}

		.field.small {
			flex: 1;
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
