<script lang="ts">
    import { page } from '$app/stores';
    import { getDebts, type DebtSummary } from "$lib/api";

    let debts: DebtSummary | null = $state(null);
    let loading = $state(true);
    let error = $state('');

    let projectId = $derived(parseInt($page.params.id ?? ''));

    async function loadDebts() {
        loading = true;
        error = '';
        try {
            debts = await getDebts(projectId);
        } catch (e) {
            error = e instanceof Error ? e.message : 'Failed to load debts';
        } finally {
            loading = false;
        }
    }

    $effect(() => {
        if (projectId) {
            loadDebts();
        }
    });
</script>

<h2>Debts Summary</h2>

{#if error}
    <div class="error">{error}</div>
{/if}

{#if loading}
    <p>Loading...</p>
{:else if debts}
    <section class="card">
        <h3>Balances</h3>
        <table class="balance-table">
            <thead>
                <tr>
                    <th>Participant</th>
                    <th>Paid</th>
                    <th>Owes</th>
                    <th>Balance</th>
                </tr>
            </thead>
            <tbody>
                {#each debts.balances as b}
                    <tr>
                        <td>{b.participant_name}</td>
                        <td>${b.total_paid.toFixed(2)}</td>
                        <td>${b.total_owed.toFixed(2)}</td>
                        <td class:positive={b.net_balance > 0} class:negative={b.net_balance < 0}>
                            {b.net_balance >= 0 ? '+' : ''}${b.net_balance.toFixed(2)}
                        </td>
                    </tr>
                {/each}
            </tbody>
        </table>
    </section>

    <section class="card">
        <h3>Settlements</h3>
        {#if debts.settlements.length === 0}
            <p class="all-settled">All settled up!</p>
        {:else}
            <ul class="settlements-list">
                {#each debts.settlements as s}
                    <li>
                        <span class="from">{s.from_participant_name}</span>
                        <span class="arrow">&rarr;</span>
                        <span class="to">{s.to_participant_name}</span>
                        <span class="amount">${s.amount.toFixed(2)}</span>
                    </li>
                {/each}
            </ul>
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

    .balance-table {
        width: 100%;
        border-collapse: collapse;
    }

    .balance-table th,
    .balance-table td {
        padding: 0.75rem;
        text-align: left;
        border-bottom: 1px solid #eee;
    }

    .balance-table th {
        font-weight: 600;
        color: #666;
    }

    .positive {
        color: #2a9d8f;
        font-weight: 600;
    }

    .negative {
        color: #e76f51;
        font-weight: 600;
    }

    .all-settled {
        text-align: center;
        padding: 2rem;
        color: #2a9d8f;
        font-size: 1.2rem;
        font-weight: 600;
    }

    .settlements-list {
        list-style: none;
        padding: 0;
        margin: 0;
    }

    .settlements-list li {
        display: flex;
        align-items: center;
        gap: 1rem;
        padding: 1rem;
        border-bottom: 1px solid #eee;
    }

    .settlements-list li:last-child {
        border-bottom: none;
    }

    .from {
        font-weight: 600;
        color: #e76f51;
    }

    .arrow {
        color: #999;
    }

    .to {
        font-weight: 600;
        color: #2a9d8f;
    }

    .amount {
        margin-left: auto;
        font-weight: 700;
        color: var(--accent, #7b61ff);
        font-size: 1.1rem;
    }
</style>
