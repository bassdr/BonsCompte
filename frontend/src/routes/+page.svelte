<!-- frontend/src/routes/+page.svelte -->
<script lang="ts">
    import { getMembers, getExpenses, postExpense, uploadReceipt } from "$lib/api";
    import { computeShares } from "$lib/utils";

    // Local state (Svelte 5 Runes)
    let members = $state([]);
    let expenses = $state([]);

    // Add expense fields
    let amount = $state("");
    let description = $state("");
    let note = $state("");
    let file: File | null = $state(null);
    let receiptUrl = $state("");

    let splits = $derived(
        members.map((m) => ({
            member_id: m.id,
            weight: m.weight ?? 1,
            included: true
        }))
    );

    let shares = $derived(() =>
        amount ? computeShares(parseFloat(amount), splits) : {}
    );

    async function loadAll() {
        members = await getMembers();
        expenses = await getExpenses();
    }

    async function doUpload(e: Event) {
        const target = e.target as HTMLInputElement;
        if (!target.files?.length) return;
        file = target.files[0];

        const res = await uploadReceipt(file);
        receiptUrl = res.url;
    }

    async function addExpense() {
        const payload = {
            amount: parseFloat(amount),
            description,
            note,
            receipt_url: receiptUrl,
            splits
        };

        await postExpense(payload);

        amount = "";
        description = "";
        note = "";
        file = null;
        receiptUrl = "";

        await loadAll();
    }

    // load on mount
    $effect(() => {
        loadAll();
    });
</script>

<h1>Shared Ledger</h1>

<section>
    <h2>Members</h2>
    {#if members.length === 0}
        <p>No members found.</p>
    {:else}
        <ul>
            {#each members as m}
                <li>{m.name} — weight: {m.weight}</li>
            {/each}
        </ul>
    {/if}
</section>

<section>
    <h2>Add Expense</h2>

    <div>
        <label>Amount</label>
        <input type="number" bind:value={amount} min="0" step="0.01" />
    </div>

    <div>
        <label>Description</label>
        <input type="text" bind:value={description} />
    </div>

    <div>
        <label>Note (optional)</label>
        <textarea bind:value={note}></textarea>
    </div>

    <div>
        <label>Receipt (optional)</label>
        <input type="file" accept="image/*" on:change={doUpload} />
        {#if receiptUrl}
            <p>Uploaded: {receiptUrl}</p>
        {/if}
    </div>

    <h3>Split</h3>
    <table>
        <thead>
        <tr>
            <th>Member</th>
            <th>Included</th>
            <th>Weight</th>
            <th>Share</th>
        </tr>
        </thead>
        <tbody>
        {#each splits as s, i}
            <tr>
                <td>{members[i].name}</td>
                <td>
                    <input type="checkbox" bind:checked={s.included} />
                </td>
                <td>
                    <input type="number" bind:value={s.weight} min="0" />
                </td>
                <td>{shares[s.member_id] ?? 0}</td>
            </tr>
        {/each}
        </tbody>
    </table>

    <button on:click={addExpense}>Add</button>
</section>

<section>
    <h2>Recent Expenses</h2>
    {#if expenses.length === 0}
        <p>No expenses yet.</p>
    {:else}
        <ul>
            {#each expenses as e}
                <li>
                    {e.description} — ${e.amount}
                    {#if e.receipt_url}
                        <br />
                        <img src={e.receipt_url} alt="receipt" width="120" />
                    {/if}
                </li>
            {/each}
        </ul>
    {/if}
</section>
