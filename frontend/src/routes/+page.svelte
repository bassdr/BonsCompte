<script lang="ts">
    import { getProjects, createProject, joinProject, type ProjectWithRole } from "$lib/api";
    import { goto } from "$app/navigation";

    let projects: ProjectWithRole[] = $state([]);
    let loading = $state(true);
    let error = $state('');

    // Create project form
    let showCreateForm = $state(false);
    let newProjectName = $state('');
    let newProjectDescription = $state('');
    let creating = $state(false);

    // Join project form
    let showJoinForm = $state(false);
    let inviteCode = $state('');
    let joining = $state(false);

    async function loadProjects() {
        loading = true;
        error = '';
        try {
            projects = await getProjects();
        } catch (e) {
            error = e instanceof Error ? e.message : 'Failed to load projects';
        } finally {
            loading = false;
        }
    }

    async function handleCreate(e: Event) {
        e.preventDefault();
        if (!newProjectName.trim()) return;

        creating = true;
        error = '';

        try {
            const project = await createProject({
                name: newProjectName.trim(),
                description: newProjectDescription.trim() || undefined
            });

            // Reset form
            newProjectName = '';
            newProjectDescription = '';
            showCreateForm = false;

            // Go to new project
            goto(`/projects/${project.id}`);
        } catch (e) {
            error = e instanceof Error ? e.message : 'Failed to create project';
        } finally {
            creating = false;
        }
    }

    async function handleJoin(e: Event) {
        e.preventDefault();
        if (!inviteCode.trim()) return;

        joining = true;
        error = '';

        try {
            const project = await joinProject(inviteCode.trim());

            // Reset form
            inviteCode = '';
            showJoinForm = false;

            // Go to joined project
            goto(`/projects/${project.id}`);
        } catch (e) {
            error = e instanceof Error ? e.message : 'Failed to join project';
        } finally {
            joining = false;
        }
    }

    function getRoleBadge(role: string): string {
        switch (role) {
            case 'admin': return 'Admin';
            case 'editor': return 'Editor';
            case 'reader': return 'Reader';
            default: return role;
        }
    }

    $effect(() => {
        loadProjects();
    });
</script>

<h1>My Projects</h1>

{#if error}
    <div class="error">{error}</div>
{/if}

<div class="actions">
    <button class="btn-primary" onclick={() => showCreateForm = !showCreateForm}>
        {showCreateForm ? 'Cancel' : '+ New Project'}
    </button>
    <button class="btn-secondary" onclick={() => showJoinForm = !showJoinForm}>
        {showJoinForm ? 'Cancel' : 'Join Project'}
    </button>
</div>

{#if showCreateForm}
    <div class="card form-card">
        <h2>Create New Project</h2>
        <form onsubmit={handleCreate}>
            <div class="field">
                <label for="name">Project Name</label>
                <input
                    id="name"
                    type="text"
                    bind:value={newProjectName}
                    placeholder="e.g., Summer Vacation 2025"
                    required
                />
            </div>
            <div class="field">
                <label for="description">Description (optional)</label>
                <input
                    id="description"
                    type="text"
                    bind:value={newProjectDescription}
                    placeholder="What's this project about?"
                />
            </div>
            <button type="submit" disabled={creating}>
                {creating ? 'Creating...' : 'Create Project'}
            </button>
        </form>
    </div>
{/if}

{#if showJoinForm}
    <div class="card form-card">
        <h2>Join a Project</h2>
        <form onsubmit={handleJoin}>
            <div class="field">
                <label for="code">Invite Code</label>
                <input
                    id="code"
                    type="text"
                    bind:value={inviteCode}
                    placeholder="Enter the invite code"
                    required
                />
            </div>
            <button type="submit" disabled={joining}>
                {joining ? 'Joining...' : 'Join Project'}
            </button>
        </form>
    </div>
{/if}

{#if loading}
    <p>Loading projects...</p>
{:else if projects.length === 0}
    <div class="empty-state">
        <p>You don't have any projects yet.</p>
        <p>Create a new project or join an existing one with an invite code.</p>
    </div>
{:else}
    <div class="projects-grid">
        {#each projects as project}
            <a href="/projects/{project.id}" class="project-card">
                <div class="project-header">
                    <h3>{project.name}</h3>
                    <span class="role-badge role-{project.role}">{getRoleBadge(project.role)}</span>
                </div>
                {#if project.description}
                    <p class="project-description">{project.description}</p>
                {/if}
                <div class="project-meta">
                    Created {new Date(project.created_at).toLocaleDateString()}
                </div>
            </a>
        {/each}
    </div>
{/if}

<style>
    h1 {
        margin-bottom: 1.5rem;
    }

    .error {
        background: #fee;
        color: #c00;
        padding: 0.75rem;
        border-radius: 8px;
        margin-bottom: 1rem;
    }

    .actions {
        display: flex;
        gap: 1rem;
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

    .btn-secondary {
        padding: 0.75rem 1.5rem;
        background: white;
        color: var(--accent, #7b61ff);
        border: 2px solid var(--accent, #7b61ff);
        border-radius: 8px;
        font-size: 1rem;
        cursor: pointer;
    }

    .btn-primary:hover, .btn-secondary:hover {
        opacity: 0.9;
    }

    .card {
        background: rgba(255, 255, 255, 0.8);
        backdrop-filter: blur(10px);
        border-radius: 16px;
        padding: 1.5rem;
        margin-bottom: 1.5rem;
        box-shadow: 0 4px 20px rgba(0, 0, 0, 0.05);
    }

    .form-card h2 {
        margin-top: 0;
        margin-bottom: 1rem;
        color: var(--accent, #7b61ff);
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

    .empty-state {
        text-align: center;
        padding: 3rem;
        color: #666;
    }

    .projects-grid {
        display: grid;
        grid-template-columns: repeat(auto-fill, minmax(300px, 1fr));
        gap: 1.5rem;
    }

    .project-card {
        display: block;
        background: rgba(255, 255, 255, 0.8);
        backdrop-filter: blur(10px);
        border-radius: 16px;
        padding: 1.5rem;
        box-shadow: 0 4px 20px rgba(0, 0, 0, 0.05);
        text-decoration: none;
        color: inherit;
        transition: transform 0.2s, box-shadow 0.2s;
    }

    .project-card:hover {
        transform: translateY(-2px);
        box-shadow: 0 8px 30px rgba(0, 0, 0, 0.1);
    }

    .project-header {
        display: flex;
        justify-content: space-between;
        align-items: flex-start;
        margin-bottom: 0.5rem;
    }

    .project-header h3 {
        margin: 0;
        color: var(--accent, #7b61ff);
    }

    .role-badge {
        font-size: 0.75rem;
        padding: 0.25rem 0.5rem;
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

    .project-description {
        color: #666;
        margin: 0.5rem 0;
        font-size: 0.9rem;
    }

    .project-meta {
        font-size: 0.8rem;
        color: #999;
        margin-top: 1rem;
    }
</style>
