<script lang="ts">
    import { page } from '$app/stores';
    import { goto } from '$app/navigation';
    import { joinProject } from '$lib/api';
    import { t } from 'svelte-i18n';

    let error = $state('');
    let joining = $state(false);
    let success = $state('');

    // Get parameters from URL
    let code = $derived($page.url.searchParams.get('code') || '');
    let participantToken = $derived($page.url.searchParams.get('p') || undefined);

    // Manual input for code if not in URL
    let manualCode = $state('');

    async function handleJoin(e: Event) {
        e.preventDefault();
        const inviteCode = code || manualCode.trim();
        if (!inviteCode) return;

        joining = true;
        error = '';
        success = '';

        try {
            const response = await joinProject(inviteCode, participantToken);

            if (response.status === 'pending') {
                success = $t('join.pendingApproval', { values: { name: response.project.name } });
            } else {
                // Go to joined project
                goto(`/projects/${response.project.id}`);
            }
        } catch (e) {
            error = e instanceof Error ? e.message : $t('join.failedToJoin');
        } finally {
            joining = false;
        }
    }
</script>

<div class="join-container">
    <h1>{$t('join.title')}</h1>

    {#if success}
        <div class="success">
            <p>{success}</p>
            <a href="/" class="btn-primary">{$t('join.goToProjects')}</a>
        </div>
    {:else}
        {#if error}
            <div class="error">{error}</div>
        {/if}

        {#if code}
            <div class="invite-info">
                <p>{$t('join.inviteMessage')}</p>
                {#if participantToken}
                    <p class="note">{$t('join.participantLinkNote')}</p>
                {/if}
            </div>

            <form onsubmit={handleJoin}>
                <div class="code-display">
                    <span class="label">{$t('join.inviteCodeLabel')}</span>
                    <code>{code}</code>
                </div>
                <button type="submit" disabled={joining}>
                    {joining ? $t('join.joining') : $t('join.joinButton')}
                </button>
            </form>
        {:else}
            <p>{$t('join.enterCodePrompt')}</p>

            <form onsubmit={handleJoin}>
                <div class="field">
                    <label for="code">{$t('join.inviteCodeInput')}</label>
                    <input
                        id="code"
                        type="text"
                        bind:value={manualCode}
                        required
                        disabled={joining}
                        placeholder={$t('join.inviteCodePlaceholder')}
                    />
                </div>

                <button type="submit" disabled={joining || !manualCode.trim()}>
                    {joining ? $t('join.joining') : $t('join.joinButton')}
                </button>
            </form>
        {/if}

        <p class="link">
            <a href="/">{$t('join.backToProjects')}</a>
        </p>
    {/if}
</div>

<style>
    .join-container {
        max-width: 500px;
        margin: 2rem auto;
        padding: 2rem;
    }

    h1 {
        text-align: center;
        margin-bottom: 2rem;
    }

    .invite-info {
        background: #e8f4fd;
        padding: 1rem;
        border-radius: 8px;
        margin-bottom: 1.5rem;
    }

    .invite-info p {
        margin: 0 0 0.5rem 0;
    }

    .invite-info p:last-child {
        margin-bottom: 0;
    }

    .note {
        font-size: 0.9rem;
        color: #666;
    }

    .code-display {
        background: #f5f5f5;
        padding: 1rem;
        border-radius: 8px;
        margin-bottom: 1.5rem;
        display: flex;
        align-items: center;
        gap: 1rem;
    }

    .code-display .label {
        color: #666;
    }

    .code-display code {
        font-size: 1.2rem;
        font-weight: 600;
        letter-spacing: 0.1em;
        color: var(--accent, #7b61ff);
    }

    .field {
        margin-bottom: 1rem;
    }

    label {
        display: block;
        margin-bottom: 0.5rem;
        font-weight: 600;
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

    button {
        width: 100%;
        padding: 0.75rem;
        margin-top: 1rem;
        background: var(--accent, #7b61ff);
        color: white;
        border: none;
        border-radius: 8px;
        font-size: 1rem;
        cursor: pointer;
    }

    button:hover:not(:disabled) {
        opacity: 0.9;
    }

    button:disabled {
        opacity: 0.6;
        cursor: not-allowed;
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
        padding: 1.5rem;
        border-radius: 8px;
        text-align: center;
    }

    .success p {
        margin: 0 0 1rem 0;
    }

    .btn-primary {
        display: inline-block;
        padding: 0.75rem 1.5rem;
        background: var(--accent, #7b61ff);
        color: white;
        text-decoration: none;
        border-radius: 8px;
    }

    .link {
        text-align: center;
        margin-top: 1.5rem;
    }

    .link a {
        color: var(--accent, #7b61ff);
    }

    /* Mobile responsive */
    @media (max-width: 768px) {
        .join-container {
            margin: 1rem auto;
            padding: 1.5rem 1rem;
        }

        h1 {
            font-size: 1.5rem;
        }

        input {
            padding: 0.875rem;
            font-size: 16px;
        }
    }

    @media (max-width: 480px) {
        .join-container {
            margin: 0;
            padding: 1rem;
            max-width: 100%;
        }

        h1 {
            font-size: 1.3rem;
        }

        .code-display {
            flex-direction: column;
            align-items: flex-start;
            gap: 0.5rem;
        }
    }
</style>
