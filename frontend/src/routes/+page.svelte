<script lang="ts">
    import { getUsers, getPayments, createPayment, deletePayment, type CreatePaymentInput } from "$lib/api";
    import { auth } from "$lib/auth";

    interface User {
        id: number;
        username: string;
        display_name: string | null;
    }

    interface Contribution {
        user_id: number;
        amount: number;
        weight: number;
    }

    interface Payment {
        id: number;
        payer_id: number | null;
        amount: number;
        description: string;
        payment_date: string;
        contributions: Contribution[];
    }

    // Data
    let users: User[] = $state([]);
    let payments: Payment[] = $state([]);
    let loading = $state(true);
    let error = $state('');

    // Form state
    let amount = $state('');
    let description = $state('');
    let payerId = $state<number | null>(null);
    let submitting = $state(false);

    // Contribution weights per user
    let weights: Record<number, number> = $state({});
    let included: Record<number, boolean> = $state({});

    // Computed shares
    let shares = $derived.by(() => {
        if (!amount || parseFloat(amount) <= 0) return {} as Record<number, number>;

        const total = parseFloat(amount);
        const activeUsers = users.filter(u => included[u.id] !== false);
        const totalWeight = activeUsers.reduce((sum, u) => sum + (weights[u.id] ?? 1), 0);

        if (totalWeight === 0) return {} as Record<number, number>;

        const result: Record<number, number> = {};
        for (const u of users) {
            if (included[u.id] === false) {
                result[u.id] = 0;
            } else {
                const w = weights[u.id] ?? 1;
                result[u.id] = Math.round((total * w / totalWeight) * 100) / 100;
            }
        }
        return result;
    });

    async function loadData() {
        loading = true;
        error = '';
        try {
            [users, payments] = await Promise.all([getUsers(), getPayments()]);
            // Initialize weights and included
            for (const u of users) {
                if (weights[u.id] === undefined) weights[u.id] = 1;
                if (included[u.id] === undefined) included[u.id] = true;
            }
            // Default payer to current user
            if (payerId === null && $auth.user) {
                const currentUser = users.find(u => u.username === $auth.user?.username);
                if (currentUser) payerId = currentUser.id;
            }
        } catch (e) {
            error = e instanceof Error ? e.message : 'Failed to load data';
        } finally {
            loading = false;
        }
    }

    async function handleSubmit(e: Event) {
        e.preventDefault();
        if (!amount || parseFloat(amount) <= 0) return;

        submitting = true;
        error = '';

        try {
            const contributions = users
                .filter(u => included[u.id] !== false)
                .map(u => ({
                    user_id: u.id,
                    weight: weights[u.id] ?? 1
                }));

            const payload: CreatePaymentInput = {
                payer_id: payerId,
                amount: parseFloat(amount),
                description,
                contributions
            };

            await createPayment(payload);

            // Reset form
            amount = '';
            description = '';

            // Reload data
            await loadData();
        } catch (e) {
            error = e instanceof Error ? e.message : 'Failed to create payment';
        } finally {
            submitting = false;
        }
    }

    async function handleDelete(id: number) {
        if (!confirm('Delete this payment?')) return;

        try {
            await deletePayment(id);
            await loadData();
        } catch (e) {
            error = e instanceof Error ? e.message : 'Failed to delete payment';
        }
    }

    function getUserName(id: number | null): string {
        if (id === null) return 'External';
        const user = users.find(u => u.id === id);
        return user?.display_name || user?.username || 'Unknown';
    }

    function formatDate(dateStr: string): string {
        return new Date(dateStr).toLocaleDateString();
    }

    $effect(() => {
        loadData();
    });
</script>

<h1>Shared Ledger</h1>

{#if error}
    <div class="error">{error}</div>
{/if}

{#if loading}
    <p>Loading...</p>
{:else}
    <section class="card">
        <h2>Add Payment</h2>

        <form onsubmit={handleSubmit}>
            <div class="form-row">
                <div class="field">
                    <label for="payer">Paid by</label>
                    <select id="payer" bind:value={payerId}>
                        {#each users as u}
                            <option value={u.id}>{u.display_name || u.username}</option>
                        {/each}
                    </select>
                </div>

                <div class="field">
                    <label for="amount">Amount</label>
                    <input
                        id="amount"
                        type="number"
                        bind:value={amount}
                        min="0.01"
                        step="0.01"
                        required
                    />
                </div>
            </div>

            <div class="field">
                <label for="description">Description</label>
                <input
                    id="description"
                    type="text"
                    bind:value={description}
                    placeholder="What was this for?"
                    required
                />
            </div>

            <h3>Split between</h3>
            <table class="split-table">
                <thead>
                    <tr>
                        <th>Member</th>
                        <th>Include</th>
                        <th>Weight</th>
                        <th>Share</th>
                    </tr>
                </thead>
                <tbody>
                    {#each users as u}
                        <tr>
                            <td>{u.display_name || u.username}</td>
                            <td>
                                <input
                                    type="checkbox"
                                    bind:checked={included[u.id]}
                                />
                            </td>
                            <td>
                                <input
                                    type="number"
                                    bind:value={weights[u.id]}
                                    min="0"
                                    step="0.5"
                                    disabled={!included[u.id]}
                                />
                            </td>
                            <td class="share">${shares[u.id]?.toFixed(2) ?? '0.00'}</td>
                        </tr>
                    {/each}
                </tbody>
            </table>

            <button type="submit" disabled={submitting}>
                {submitting ? 'Adding...' : 'Add Payment'}
            </button>
        </form>
    </section>

    <section class="card">
        <h2>Recent Payments</h2>

        {#if payments.length === 0}
            <p class="empty">No payments yet.</p>
        {:else}
            <ul class="payments-list">
                {#each payments as p}
                    <li>
                        <div class="payment-header">
                            <strong>{p.description}</strong>
                            <span class="amount">${p.amount.toFixed(2)}</span>
                        </div>
                        <div class="payment-meta">
                            Paid by {getUserName(p.payer_id)} on {formatDate(p.payment_date)}
                        </div>
                        <div class="payment-splits">
                            {#each p.contributions as c}
                                <span class="chip">
                                    {getUserName(c.user_id)}: ${c.amount.toFixed(2)}
                                </span>
                            {/each}
                        </div>
                        <button class="delete-btn" onclick={() => handleDelete(p.id)}>
                            Delete
                        </button>
                    </li>
                {/each}
            </ul>
        {/if}
    </section>
{/if}

<style>
    h1 {
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

    h2 {
        margin-top: 0;
        margin-bottom: 1rem;
        color: var(--accent, #7b61ff);
    }

    h3 {
        margin: 1rem 0 0.5rem;
        font-size: 1rem;
    }

    .error {
        background: #fee;
        color: #c00;
        padding: 0.75rem;
        border-radius: 8px;
        margin-bottom: 1rem;
    }

    .form-row {
        display: flex;
        gap: 1rem;
    }

    .field {
        flex: 1;
        margin-bottom: 1rem;
    }

    label {
        display: block;
        margin-bottom: 0.5rem;
        font-weight: 600;
        font-size: 0.9rem;
    }

    input, select {
        width: 100%;
        padding: 0.75rem;
        border: 1px solid #ddd;
        border-radius: 8px;
        font-size: 1rem;
    }

    input:focus, select:focus {
        outline: none;
        border-color: var(--accent, #7b61ff);
    }

    .split-table {
        width: 100%;
        border-collapse: collapse;
        margin-bottom: 1rem;
    }

    .split-table th,
    .split-table td {
        padding: 0.5rem;
        text-align: left;
        border-bottom: 1px solid #eee;
    }

    .split-table input[type="number"] {
        width: 80px;
        padding: 0.5rem;
    }

    .split-table input[type="checkbox"] {
        width: auto;
    }

    .share {
        font-weight: 600;
        color: var(--accent, #7b61ff);
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

    button[type="submit"]:hover:not(:disabled) {
        opacity: 0.9;
    }

    button[type="submit"]:disabled {
        opacity: 0.6;
        cursor: not-allowed;
    }

    .payments-list {
        list-style: none;
        padding: 0;
        margin: 0;
    }

    .payments-list li {
        padding: 1rem;
        border-bottom: 1px solid #eee;
        position: relative;
    }

    .payments-list li:last-child {
        border-bottom: none;
    }

    .payment-header {
        display: flex;
        justify-content: space-between;
        align-items: center;
        margin-bottom: 0.25rem;
    }

    .amount {
        font-weight: 700;
        color: var(--accent, #7b61ff);
    }

    .payment-meta {
        font-size: 0.85rem;
        color: #666;
        margin-bottom: 0.5rem;
    }

    .payment-splits {
        display: flex;
        flex-wrap: wrap;
        gap: 0.5rem;
    }

    .chip {
        background: #f0f0f0;
        padding: 0.25rem 0.5rem;
        border-radius: 4px;
        font-size: 0.85rem;
    }

    .delete-btn {
        position: absolute;
        top: 1rem;
        right: 1rem;
        padding: 0.25rem 0.5rem;
        background: transparent;
        border: 1px solid #ddd;
        border-radius: 4px;
        font-size: 0.75rem;
        color: #999;
        cursor: pointer;
    }

    .delete-btn:hover {
        background: #fee;
        border-color: #fcc;
        color: #c00;
    }

    .empty {
        color: #666;
        font-style: italic;
    }
</style>
