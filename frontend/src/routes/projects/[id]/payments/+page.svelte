<script lang="ts">
    import { page } from '$app/stores';
    import { getPayments, createPayment, updatePayment, deletePayment, type PaymentWithContributions, type CreatePaymentInput } from "$lib/api";
    import { participants, currentProject, canEdit } from '$lib/stores/project';

    let payments: PaymentWithContributions[] = $state([]);
    let loading = $state(true);
    let error = $state('');

    // Edit mode state
    let editingPaymentId = $state<number | null>(null);

    // Form state
    let amount = $state('');
    let description = $state('');
    let payerId = $state<number | null>(null);
    let paymentDate = $state(getLocalDateString());
    let submitting = $state(false);

    // Receipt image state
    let receiptImage = $state<string | null>(null);
    let receiptPreview = $state<string | null>(null);

    // Recurrence state
    let isRecurring = $state(false);
    let recurrenceMode = $state<'every' | 'times_per'>('every');
    let recurrenceInterval = $state(1);
    let recurrenceType = $state<'daily' | 'weekly' | 'monthly' | 'yearly'>('monthly');
    let recurrenceTimesPer = $state(1);
    let recurrenceEndDate = $state('');

    // Image modal state
    let showImageModal = $state(false);
    let modalImage = $state<string | null>(null);

    // Contribution weights per participant
    let weights: Record<number, number> = $state({});
    let included: Record<number, boolean> = $state({});

    // Helper functions for local date handling
    function getLocalDateString(date: Date = new Date()): string {
        const year = date.getFullYear();
        const month = String(date.getMonth() + 1).padStart(2, '0');
        const day = String(date.getDate()).padStart(2, '0');
        return `${year}-${month}-${day}`;
    }

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
        if ($participants.length > 0 && editingPaymentId === null) {
            for (const p of $participants) {
                if (weights[p.id] === undefined) weights[p.id] = p.default_weight;
                if (included[p.id] === undefined) included[p.id] = p.default_weight > 0;
            }
            // Default payer to first participant
            if (payerId === null && $participants.length > 0) {
                payerId = $participants[0].id;
            }
        }
    });

    // Sync weight and included: weight 0 unchecks, uncheck sets weight to 0
    function handleWeightChange(participantId: number, newWeight: number) {
        weights[participantId] = newWeight;
        if (newWeight === 0) {
            included[participantId] = false;
        }
    }

    function handleIncludedChange(participantId: number, isIncluded: boolean) {
        included[participantId] = isIncluded;
        if (!isIncluded) {
            weights[participantId] = 0;
        } else if (weights[participantId] === 0) {
            // Restore to default weight when re-including
            const participant = $participants.find(p => p.id === participantId);
            weights[participantId] = participant?.default_weight || 1;
        }
    }

    // Pay this person back - uncheck all except this one
    function payBack(participantId: number) {
        for (const p of $participants) {
            if (p.id === participantId) {
                included[p.id] = true;
                weights[p.id] = 1;
            } else {
                included[p.id] = false;
                weights[p.id] = 0;
            }
        }
    }

    // Include all participants with default weight > 0
    function includeAll() {
        for (const p of $participants) {
            if (p.default_weight > 0) {
                included[p.id] = true;
                weights[p.id] = p.default_weight;
            }
        }
    }

    // Handle file upload for receipt
    function handleFileChange(event: Event) {
        const input = event.target as HTMLInputElement;
        const file = input.files?.[0];
        if (!file) {
            receiptImage = null;
            receiptPreview = null;
            return;
        }

        // Validate file type
        if (!file.type.startsWith('image/')) {
            error = 'Please select an image file';
            return;
        }

        // Validate file size (max 5MB)
        if (file.size > 5 * 1024 * 1024) {
            error = 'Image must be less than 5MB';
            return;
        }

        const reader = new FileReader();
        reader.onload = (e) => {
            const base64 = e.target?.result as string;
            receiptImage = base64;
            receiptPreview = base64;
        };
        reader.readAsDataURL(file);
    }

    function clearReceipt() {
        receiptImage = null;
        receiptPreview = null;
    }

    // Load payments when projectId changes
    $effect(() => {
        if (projectId) {
            loadPayments();
        }
    });

    // Reset form to default state
    function resetForm() {
        editingPaymentId = null;
        amount = '';
        description = '';
        paymentDate = getLocalDateString();
        receiptImage = null;
        receiptPreview = null;
        isRecurring = false;
        recurrenceMode = 'every';
        recurrenceInterval = 1;
        recurrenceType = 'monthly';
        recurrenceTimesPer = 1;
        recurrenceEndDate = '';

        // Reset weights and included to defaults
        for (const p of $participants) {
            weights[p.id] = p.default_weight;
            included[p.id] = p.default_weight > 0;
        }

        // Reset payer
        if ($participants.length > 0) {
            payerId = $participants[0].id;
        }
    }

    // Start editing a payment
    function startEditing(payment: PaymentWithContributions) {
        editingPaymentId = payment.id;
        amount = payment.amount.toString();
        description = payment.description;
        payerId = payment.payer_id;
        paymentDate = payment.payment_date.split('T')[0]; // Handle ISO date format
        receiptImage = payment.receipt_image;
        receiptPreview = payment.receipt_image;
        isRecurring = payment.is_recurring;

        if (payment.is_recurring) {
            recurrenceType = (payment.recurrence_type as 'daily' | 'weekly' | 'monthly' | 'yearly') || 'monthly';
            if (payment.recurrence_times_per) {
                recurrenceMode = 'times_per';
                recurrenceTimesPer = payment.recurrence_times_per;
                recurrenceInterval = 1;
            } else {
                recurrenceMode = 'every';
                recurrenceInterval = payment.recurrence_interval || 1;
            }
            recurrenceEndDate = payment.recurrence_end_date || '';
        }

        // Set weights and included from contributions
        for (const p of $participants) {
            const contrib = payment.contributions.find(c => c.participant_id === p.id);
            if (contrib) {
                included[p.id] = true;
                weights[p.id] = contrib.weight;
            } else {
                included[p.id] = false;
                weights[p.id] = 0;
            }
        }
    }

    function cancelEditing() {
        resetForm();
    }

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
                payment_date: paymentDate,
                contributions,
                receipt_image: receiptImage ?? undefined,
                is_recurring: isRecurring,
            };

            if (isRecurring) {
                payload.recurrence_type = recurrenceType;
                if (recurrenceMode === 'every') {
                    payload.recurrence_interval = recurrenceInterval;
                } else {
                    payload.recurrence_times_per = recurrenceTimesPer;
                    payload.recurrence_interval = 1;
                }
                if (recurrenceEndDate) {
                    payload.recurrence_end_date = recurrenceEndDate;
                }
            }

            if (editingPaymentId !== null) {
                await updatePayment(projectId, editingPaymentId, payload);
            } else {
                await createPayment(projectId, payload);
            }

            // Reset form
            resetForm();

            // Reload data
            await loadPayments();
        } catch (e) {
            error = e instanceof Error ? e.message : 'Failed to save payment';
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
        const [year, month, day] = dateStr.split('T')[0].split('-').map(Number);
        const date = new Date(year, month - 1, day);
        return date.toLocaleDateString();
    }

    function isFutureDate(dateStr: string): boolean {
        const [year, month, day] = dateStr.split('T')[0].split('-').map(Number);
        const date = new Date(year, month - 1, day);
        const today = new Date();
        today.setHours(0, 0, 0, 0);
        return date > today;
    }

    function formatRecurrence(p: PaymentWithContributions): string {
        if (!p.is_recurring) return '';
        const type = p.recurrence_type || 'monthly';
        const interval = p.recurrence_interval || 1;
        const timesPer = p.recurrence_times_per;

        if (timesPer) {
            return `${timesPer}x per ${type.replace('ly', '')}`;
        }
        if (interval === 1) {
            return type;
        }
        return `every ${interval} ${type.replace('ly', '')}s`;
    }

    function openImageModal(image: string) {
        modalImage = image;
        showImageModal = true;
    }

    function closeImageModal() {
        showImageModal = false;
        modalImage = null;
    }
</script>

<h2>Payments</h2>

{#if error}
    <div class="error">{error}</div>
{/if}

{#if $canEdit}
    <section class="card">
        <h3>{editingPaymentId !== null ? 'Edit Payment' : 'Add Payment'}</h3>

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

                    <div class="field">
                        <label for="payment-date">Date</label>
                        <input
                            id="payment-date"
                            type="date"
                            bind:value={paymentDate}
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

                <!-- Receipt Image -->
                <div class="field">
                    <label>Receipt Image (optional)</label>
                    <div class="receipt-upload">
                        <input
                            type="file"
                            accept="image/*"
                            onchange={handleFileChange}
                            id="receipt-input"
                        />
                        {#if receiptPreview}
                            <div class="receipt-preview">
                                <img src={receiptPreview} alt="Receipt preview" />
                                <button type="button" class="clear-btn" onclick={clearReceipt}>&times;</button>
                            </div>
                        {/if}
                    </div>
                </div>

                <!-- Recurrence Section -->
                <div class="recurrence-section">
                    <label class="checkbox-label">
                        <input type="checkbox" bind:checked={isRecurring} />
                        Recurring payment
                    </label>

                    {#if isRecurring}
                        <div class="recurrence-options">
                            <div class="form-row">
                                <div class="field">
                                    <label>Mode</label>
                                    <select bind:value={recurrenceMode}>
                                        <option value="every">Every X period</option>
                                        <option value="times_per">X times per period</option>
                                    </select>
                                </div>

                                {#if recurrenceMode === 'every'}
                                    <div class="field small">
                                        <label>Every</label>
                                        <input type="number" bind:value={recurrenceInterval} min="1" />
                                    </div>
                                {:else}
                                    <div class="field small">
                                        <label>Times</label>
                                        <input type="number" bind:value={recurrenceTimesPer} min="1" />
                                    </div>
                                {/if}

                                <div class="field">
                                    <label>Period</label>
                                    <select bind:value={recurrenceType}>
                                        <option value="daily">{recurrenceMode === 'every' ? 'Day(s)' : 'Day'}</option>
                                        <option value="weekly">{recurrenceMode === 'every' ? 'Week(s)' : 'Week'}</option>
                                        <option value="monthly">{recurrenceMode === 'every' ? 'Month(s)' : 'Month'}</option>
                                        <option value="yearly">{recurrenceMode === 'every' ? 'Year(s)' : 'Year'}</option>
                                    </select>
                                </div>
                            </div>

                            <div class="field">
                                <label for="end-date">End date (optional)</label>
                                <input
                                    id="end-date"
                                    type="date"
                                    bind:value={recurrenceEndDate}
                                    min={paymentDate}
                                />
                            </div>
                        </div>
                    {/if}
                </div>

                <div class="split-header">
                    <h4>Split between</h4>
                    <button type="button" class="small-btn" onclick={includeAll}>Include all</button>
                </div>
                <table class="split-table">
                    <thead>
                        <tr>
                            <th>Participant</th>
                            <th>Include</th>
                            <th>Weight</th>
                            <th>Share</th>
                            <th></th>
                        </tr>
                    </thead>
                    <tbody>
                        {#each $participants as p}
                            <tr>
                                <td>{p.name}</td>
                                <td>
                                    <input
                                        type="checkbox"
                                        checked={included[p.id]}
                                        onchange={(e) => handleIncludedChange(p.id, e.currentTarget.checked)}
                                    />
                                </td>
                                <td>
                                    <input
                                        type="number"
                                        value={weights[p.id]}
                                        oninput={(e) => handleWeightChange(p.id, parseFloat(e.currentTarget.value) || 0)}
                                        min="0"
                                        step="0.5"
                                        disabled={!included[p.id]}
                                    />
                                </td>
                                <td class="share">${shares[p.id]?.toFixed(2) ?? '0.00'}</td>
                                <td>
                                    <button
                                        type="button"
                                        class="payback-btn"
                                        onclick={() => payBack(p.id)}
                                        title="Pay only this person back"
                                    >Only</button>
                                </td>
                            </tr>
                        {/each}
                    </tbody>
                </table>

                <div class="form-actions">
                    <button type="submit" disabled={submitting || $participants.length === 0}>
                        {submitting ? 'Saving...' : (editingPaymentId !== null ? 'Update Payment' : 'Add Payment')}
                    </button>
                    {#if editingPaymentId !== null}
                        <button type="button" class="cancel-btn" onclick={cancelEditing}>Cancel</button>
                    {/if}
                </div>
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
                <li class:editing={editingPaymentId === p.id}>
                    <div class="payment-header">
                        <div class="payment-title">
                            <strong>{p.description}</strong>
                            <div class="payment-icons">
                                {#if p.is_recurring}
                                    <span class="icon recurring" title={formatRecurrence(p)}>&#x21bb;</span>
                                {/if}
                                {#if p.receipt_image}
                                    <button
                                        class="icon-btn"
                                        title="View receipt"
                                        onclick={() => openImageModal(p.receipt_image!)}
                                    >&#x1F9FE;</button>
                                {/if}
                            </div>
                        </div>
                        <span class="amount-group">
                            <span class="amount">${p.amount.toFixed(2)}</span>
                            {#if $canEdit}
                                <button
                                    class="edit-btn"
                                    onclick={() => startEditing(p)}
                                    title="Edit payment"
                                    disabled={editingPaymentId !== null}
                                >&#x2699;</button>
                                <button class="delete-btn" onclick={() => handleDelete(p.id)} title="Delete payment">
                                    &times;
                                </button>
                            {/if}
                        </span>
                    </div>
                    <div class="payment-meta">
                        Paid by {p.payer_name ?? 'Unknown'} {isFutureDate(p.payment_date) ? 'from' : 'on'} {formatDate(p.payment_date)}
                        {#if p.is_recurring}
                            <span class="recurrence-badge">{formatRecurrence(p)}</span>
                        {/if}
                    </div>
                    <div class="payment-splits">
                        {#each p.contributions as c}
                            <span class="chip">
                                {c.participant_name}: ${c.amount.toFixed(2)}
                            </span>
                        {/each}
                    </div>
                </li>
            {/each}
        </ul>
    {/if}
</section>

<!-- Image Modal -->
{#if showImageModal && modalImage}
    <div class="modal-overlay" onclick={closeImageModal}>
        <div class="modal-content" onclick={(e) => e.stopPropagation()}>
            <button class="modal-close" onclick={closeImageModal}>&times;</button>
            <img src={modalImage} alt="Receipt" />
        </div>
    </div>
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

    h4 {
        margin: 0;
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
        flex-wrap: wrap;
    }

    .field {
        flex: 1;
        min-width: 120px;
        margin-bottom: 1rem;
    }

    .field.small {
        flex: 0 0 80px;
        min-width: 80px;
    }

    label {
        display: block;
        margin-bottom: 0.5rem;
        font-weight: 600;
        font-size: 0.9rem;
    }

    .checkbox-label {
        display: flex;
        align-items: center;
        gap: 0.5rem;
        cursor: pointer;
    }

    .checkbox-label input[type="checkbox"] {
        width: auto;
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

    /* Receipt upload */
    .receipt-upload {
        display: flex;
        flex-direction: column;
        gap: 0.5rem;
    }

    .receipt-preview {
        position: relative;
        display: inline-block;
        max-width: 200px;
    }

    .receipt-preview img {
        max-width: 100%;
        max-height: 150px;
        border-radius: 8px;
        border: 1px solid #ddd;
    }

    .clear-btn {
        position: absolute;
        top: -8px;
        right: -8px;
        width: 24px;
        height: 24px;
        border-radius: 50%;
        background: #c00;
        color: white;
        border: none;
        cursor: pointer;
        font-size: 14px;
        line-height: 1;
    }

    /* Recurrence section */
    .recurrence-section {
        background: #f9f9f9;
        border-radius: 8px;
        padding: 1rem;
        margin-bottom: 1rem;
    }

    .recurrence-options {
        margin-top: 1rem;
        padding-top: 1rem;
        border-top: 1px solid #eee;
    }

    .split-header {
        display: flex;
        align-items: center;
        justify-content: space-between;
        margin: 1rem 0 0.5rem;
    }

    .small-btn {
        padding: 0.25rem 0.75rem;
        background: #e0e0e0;
        border: none;
        border-radius: 4px;
        font-size: 0.8rem;
        cursor: pointer;
    }

    .small-btn:hover {
        background: #d0d0d0;
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

    .payback-btn {
        padding: 0.2rem 0.5rem;
        background: #e9e4ff;
        border: 1px solid var(--accent, #7b61ff);
        border-radius: 4px;
        font-size: 0.7rem;
        color: var(--accent, #7b61ff);
        cursor: pointer;
    }

    .payback-btn:hover {
        background: var(--accent, #7b61ff);
        color: white;
    }

    .form-actions {
        display: flex;
        gap: 0.5rem;
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

    .cancel-btn {
        padding: 0.75rem 1.5rem;
        background: #e0e0e0;
        color: #333;
        border: none;
        border-radius: 8px;
        font-size: 1rem;
        cursor: pointer;
    }

    .cancel-btn:hover {
        background: #d0d0d0;
    }

    .payments-list {
        list-style: none;
        padding: 0;
        margin: 0;
    }

    .payments-list li {
        padding: 1rem;
        border-bottom: 1px solid #eee;
    }

    .payments-list li:last-child {
        border-bottom: none;
    }

    .payments-list li.editing {
        background: #f0f8ff;
        border-radius: 8px;
    }

    .payment-header {
        display: flex;
        justify-content: space-between;
        align-items: center;
        margin-bottom: 0.25rem;
    }

    .payment-title {
        display: flex;
        align-items: center;
        gap: 0.5rem;
    }

    .payment-icons {
        display: flex;
        gap: 0.25rem;
    }

    .icon {
        font-size: 0.9rem;
        color: #666;
    }

    .icon.recurring {
        color: var(--accent, #7b61ff);
    }

    .icon-btn {
        background: none;
        border: none;
        cursor: pointer;
        font-size: 0.9rem;
        padding: 0;
    }

    .icon-btn:hover {
        opacity: 0.7;
    }

    .amount-group {
        display: flex;
        align-items: center;
        gap: 0.5rem;
    }

    .amount {
        font-weight: 700;
        color: var(--accent, #7b61ff);
    }

    .edit-btn {
        display: flex;
        align-items: center;
        justify-content: center;
        width: 20px;
        height: 20px;
        padding: 0;
        background: transparent;
        border: none;
        border-radius: 50%;
        font-size: 0.85rem;
        line-height: 1;
        color: #999;
        cursor: pointer;
    }

    .edit-btn:hover:not(:disabled) {
        background: #e9e4ff;
        color: var(--accent, #7b61ff);
    }

    .edit-btn:disabled {
        opacity: 0.3;
        cursor: not-allowed;
    }

    .payment-meta {
        font-size: 0.85rem;
        color: #666;
        margin-bottom: 0.5rem;
        display: flex;
        align-items: center;
        gap: 0.5rem;
        flex-wrap: wrap;
    }

    .recurrence-badge {
        background: var(--accent, #7b61ff);
        color: white;
        padding: 0.15rem 0.5rem;
        border-radius: 4px;
        font-size: 0.75rem;
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
        display: flex;
        align-items: center;
        justify-content: center;
        width: 20px;
        height: 20px;
        padding: 0;
        background: transparent;
        border: none;
        border-radius: 50%;
        font-size: 1rem;
        line-height: 1;
        color: #999;
        cursor: pointer;
    }

    .delete-btn:hover {
        background: #fee;
        color: #c00;
    }

    .empty {
        color: #666;
        font-style: italic;
    }

    /* Modal */
    .modal-overlay {
        position: fixed;
        top: 0;
        left: 0;
        right: 0;
        bottom: 0;
        background: rgba(0, 0, 0, 0.8);
        display: flex;
        align-items: center;
        justify-content: center;
        z-index: 1000;
    }

    .modal-content {
        position: relative;
        max-width: 90vw;
        max-height: 90vh;
    }

    .modal-content img {
        max-width: 100%;
        max-height: 90vh;
        border-radius: 8px;
    }

    .modal-close {
        position: absolute;
        top: -12px;
        right: -12px;
        width: 32px;
        height: 32px;
        border-radius: 50%;
        background: white;
        border: none;
        cursor: pointer;
        font-size: 20px;
        line-height: 1;
        box-shadow: 0 2px 8px rgba(0, 0, 0, 0.2);
    }

    .modal-close:hover {
        background: #f0f0f0;
    }
</style>
