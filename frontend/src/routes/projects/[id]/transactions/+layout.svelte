<script lang="ts">
  import { page } from '$app/stores';
  import { resolveRoute } from '$app/paths';
  import type { Snippet } from 'svelte';
  import { _ } from '$lib/i18n';

  let { children }: { children: Snippet } = $props();
</script>

<div class="transactions-layout">
  <nav class="transactions-nav">
    <a
      href={resolveRoute(`/projects/${$page.params.id}/transactions/outgoing`)}
      class:active={$page.url.pathname.includes('/outgoing')}
    >
      {$_('transactions.outgoing')}
    </a>
    <a
      href={resolveRoute(`/projects/${$page.params.id}/transactions/internal`)}
      class:active={$page.url.pathname.includes('/internal')}
    >
      {$_('transactions.internal')}
    </a>
    <a
      href={resolveRoute(`/projects/${$page.params.id}/transactions/incoming`)}
      class:active={$page.url.pathname.includes('/incoming')}
    >
      {$_('transactions.incoming')}
    </a>
  </nav>

  <div class="transactions-content">
    {@render children()}
  </div>
</div>

<style>
  .transactions-layout {
    display: flex;
    flex-direction: column;
    gap: 1rem;
  }

  .transactions-nav {
    display: flex;
    gap: 0.5rem;
    background: rgba(255, 255, 255, 0.6);
    backdrop-filter: blur(10px);
    border-radius: 10px;
    padding: 0.4rem;
    overflow-x: auto;
    -webkit-overflow-scrolling: touch;
  }

  .transactions-nav a {
    padding: 0.5rem 1rem;
    text-decoration: none;
    color: #666;
    font-weight: 500;
    font-size: 0.9rem;
    border-radius: 6px;
    transition: all 0.2s;
    white-space: nowrap;
    flex-shrink: 0;
  }

  .transactions-nav a:hover {
    background: rgba(123, 97, 255, 0.1);
    color: var(--accent, #7b61ff);
  }

  .transactions-nav a.active {
    background: var(--accent, #7b61ff);
    color: white;
  }

  .transactions-content {
    flex: 1;
  }

  /* Mobile responsive styles */
  @media (max-width: 768px) {
    .transactions-nav {
      padding: 0.35rem;
    }

    .transactions-nav a {
      padding: 0.4rem 0.8rem;
      font-size: 0.85rem;
    }
  }

  @media (max-width: 480px) {
    .transactions-nav {
      padding: 0.3rem;
      gap: 0.3rem;
    }

    .transactions-nav a {
      padding: 0.35rem 0.6rem;
      font-size: 0.8rem;
    }
  }
</style>
