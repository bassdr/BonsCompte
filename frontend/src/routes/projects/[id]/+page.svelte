<script lang="ts">
    import { page } from '$app/stores';
    import { getPayments, createPayment, deletePayment, type PaymentWithContributions, type CreatePaymentInput } from "$lib/api";
    import { participants, currentProject, canEdit } from '$lib/stores/project';

    let payments: PaymentWithContributions[] = $state([]);
    let loading = $state(true);
    let error = $state('');

    // Form state
    let amount = $state('');
    let description = $state('');
    let payerId = $state<number | null>(null);
    let submitting = $state(false);

    // Contribution weights per participant
    let weights: Record<number, number> = $state({});
    let included: Record<number, boolean> = $state({});

    // Get project ID from URL
    let projectId = $derived(parseInt($page.params.id ?? ''));

    // Computed shares
    let shares = $derived.by(() => {
        if (!amount || parseFloat(amount) <= 0) return {} as Record<number, number>;

        const total = parseFloat(amount);
        const activeParticipants = $participants.filter(p => included[p.id] !== false);
        const totalWeight = activeParticipants.reduce((sum, p) => sum + (weights[p.id] ?? p.default_weight), 0);

        if (totalWeight === 0) return {} as Record<number, number>;

        const result: Record<number, number> = {};
        for (const p of $participants) {
            if (included[p.id] === false) {
                result[p.id] = 0;
            } else {
                const w = weights[p.id] ?? p.default_weight;
                result[p.id] = Math.round((total * w / totalWeight) * 100) / 100;
            }
        }
        return result;
    });

    async function loadPayments() {
        loading = true;
        error = '';
        try {
            payments = await getPayments(projectId);
        } catch (e) {
            error = e instanceof Error ? e.message : 'Failed to load payments';
        } finally {
            loading = false;
        }
    }

    // Initialize form when participants change
    $effect(() => {
        if ($participants.length > 0) {
            for (const p of $participants) {
                if (weights[p.id] === undefined) weights[p.id] = p.default_weight;
                if (included[p.id] === undefined) included[p.id] = true;
            }
            // Default payer to first participant
            if (payerId === null && $participants.length > 0) {
                payerId = $participants[0].id;
            }
        }
    });

    // Load payments when projectId changes
    $effect(() => {
        if (projectId) {
            loadPayments();
        }
    });

    async function handleSubmit(e: Event) {
        e.preventDefault();
        if (!amount || parseFloat(amount) <= 0) return;

        submitting = true;
        error = '';

        try {
            const contributions = $participants
                .filter(p => included[p.id] !== false)
                .map(p => ({
                    participant_id: p.id,
                    weight: weights[p.id] ?? p.default_weight
                }));

            const payload: CreatePaymentInput = {
                payer_id: payerId,
                amount: parseFloat(amount),
                description,
                contributions
            };

            await createPayment(projectId, payload);

            // Reset form
            amount = '';
            description = '';

            // Reload data
            await loadPayments();
        } catch (e) {
            error = e instanceof Error ? e.message : 'Failed to create payment';
        } finally {
            submitting = false;
        }
    }

    async function handleDelete(paymentId: number) {
        if (!confirm('Delete this payment?')) return;

        try {
            await deletePayment(projectId, paymentId);
            await loadPayments();
        } catch (e) {
            error = e instanceof Error ? e.message : 'Failed to delete payment';
        }
    }

    function formatDate(dateStr: string): string {
        return new Date(dateStr).toLocaleDateString();
    }
</script>

<h2>Payments</h2>

{#if error}
    <div class="error">{error}</div>
{/if}

{#if $canEdit}
    <section class="card">
        <h3>Add Payment</h3>

        {#if $participants.length === 0}
            <p class="warning">Add participants before creating payments.</p>
        {:else}
            <form onsubmit={handleSubmit}>
                <div class="form-row">
                    <div class="field">
                        <label for="payer">Paid by</label>
                        <select id="payer" bind:value={payerId}>
                            {#each $participants as p}
                                <option value={p.id}>{p.name}</option>
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

                <h4>Split between</h4>
                <table class="split-table">
                    <thead>
                        <tr>
                            <th>Participant</th>
                            <th>Include</th>
                            <th>Weight</th>
                            <th>Share</th>
                        </tr>
                    </thead>
                    <tbody>
                        {#each $participants as p}
                            <tr>
                                <td>{p.name}</td>
                                <td>
                                    <input
                                        type="checkbox"
                                        bind:checked={included[p.id]}
                                    />
                                </td>
                                <td>
                                    <input
                                        type="number"
                                        bind:value={weights[p.id]}
                                        min="0"
                                        step="0.5"
                                        disabled={!included[p.id]}
                                    />
                                </td>
                                <td class="share">${shares[p.id]?.toFixed(2) ?? '0.00'}</td>
                            </tr>
                        {/each}
                    </tbody>
                </table>

                <button type="submit" disabled={submitting || $participants.length === 0}>
                    {submitting ? 'Adding...' : 'Add Payment'}
                </button>
            </form>
        {/if}
    </section>
{/if}

<section class="card">
    <h3>Recent Payments</h3>

    {#if loading}
        <p>Loading...</p>
    {:else if payments.length === 0}
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
                        Paid by {p.payer_name ?? 'Unknown'} on {formatDate(p.payment_date)}
                    </div>
                    <div class="payment-splits">
                        {#each p.contributions as c}
                            <span class="chip">
                                {c.participant_name}: ${c.amount.toFixed(2)}
                            </span>
                        {/each}
                    </div>
                    {#if $canEdit}
                        <button class="delete-btn" onclick={() => handleDelete(p.id)}>
                            Delete
                        </button>
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

    h4 {
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

    .warning {
        background: #fff3cd;
        color: #856404;
        padding: 0.75rem;
        border-radius: 8px;
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
