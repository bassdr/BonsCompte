<script lang="ts">
  import { _ } from 'svelte-i18n';
  import { goto } from '$app/navigation';
</script>

<div class="help-container">
  <button class="back-btn" onclick={() => goto('/login')}>
    ← {$_('common.back', { default: 'Back' })}
  </button>

  <h1>{$_('help.passwordRecovery.title')}</h1>

  <div class="content">
    <section>
      <h2>{$_('help.passwordRecovery.overview.title')}</h2>
      <p>{$_('help.passwordRecovery.overview.description')}</p>
    </section>

    <section>
      <h2>{$_('help.passwordRecovery.twoLayer.title')}</h2>

      <h3>{$_('help.passwordRecovery.twoLayer.systemAdmin.title')}</h3>
      <p>{$_('help.passwordRecovery.twoLayer.systemAdmin.description')}</p>
      <div class="code-block">
        <code>bonscompte-admin reset-password &lt;username&gt;</code>
      </div>
      <p>{$_('help.passwordRecovery.twoLayer.systemAdmin.result')}</p>

      <h3>{$_('help.passwordRecovery.twoLayer.projectMembers.title')}</h3>
      <p>{$_('help.passwordRecovery.twoLayer.projectMembers.description')}</p>

      <div class="info-box">
        <strong>{$_('help.passwordRecovery.twoLayer.projectMembers.votingRules.title')}</strong>
        <ul>
          <li>{$_('help.passwordRecovery.twoLayer.projectMembers.votingRules.admin')}</li>
          <li>{$_('help.passwordRecovery.twoLayer.projectMembers.votingRules.regular')}</li>
          <li>{$_('help.passwordRecovery.twoLayer.projectMembers.votingRules.solo')}</li>
        </ul>
      </div>
    </section>

    <section>
      <h2>{$_('help.passwordRecovery.workflow.title')}</h2>

      <div class="steps">
        <div class="step">
          <div class="step-number">1</div>
          <div class="step-content">
            <h4>{$_('help.passwordRecovery.workflow.step1.title')}</h4>
            <p>{$_('help.passwordRecovery.workflow.step1.description')}</p>
          </div>
        </div>

        <div class="step">
          <div class="step-number">2</div>
          <div class="step-content">
            <h4>{$_('help.passwordRecovery.workflow.step2.title')}</h4>
            <p>{$_('help.passwordRecovery.workflow.step2.description')}</p>
          </div>
        </div>

        <div class="step">
          <div class="step-number">3</div>
          <div class="step-content">
            <h4>{$_('help.passwordRecovery.workflow.step3.title')}</h4>
            <p>{$_('help.passwordRecovery.workflow.step3.description')}</p>
          </div>
        </div>

        <div class="step">
          <div class="step-number">4</div>
          <div class="step-content">
            <h4>{$_('help.passwordRecovery.workflow.step4.title')}</h4>
            <p>{$_('help.passwordRecovery.workflow.step4.description')}</p>
          </div>
        </div>
      </div>
    </section>

    <section>
      <h2>{$_('help.passwordRecovery.whyApproval.title')}</h2>
      <p>{$_('help.passwordRecovery.whyApproval.description')}</p>
      <ul>
        <li>{$_('help.passwordRecovery.whyApproval.reason1')}</li>
        <li>{$_('help.passwordRecovery.whyApproval.reason2')}</li>
        <li>{$_('help.passwordRecovery.whyApproval.reason3')}</li>
      </ul>
    </section>

    <section class="warning-box">
      <h2>{$_('help.passwordRecovery.important.title')}</h2>
      <ul>
        <li>{$_('help.passwordRecovery.important.point1')}</li>
        <li>{$_('help.passwordRecovery.important.point2')}</li>
        <li>{$_('help.passwordRecovery.important.point3')}</li>
      </ul>
    </section>

    <section>
      <h2>{$_('help.passwordRecovery.contactAdmin.title')}</h2>
      <p>{$_('help.passwordRecovery.contactAdmin.description')}</p>
    </section>
  </div>
</div>

<style>
  .help-container {
    max-width: 800px;
    margin: 0 auto;
    padding: 2rem 1rem;
  }

  .back-btn {
    background: none;
    border: none;
    color: #3b82f6;
    cursor: pointer;
    font-size: 0.875rem;
    padding: 0.5rem 0;
    margin-bottom: 1rem;
  }

  .back-btn:hover {
    text-decoration: underline;
  }

  h1 {
    color: #111827;
    margin-bottom: 2rem;
    font-size: 2rem;
  }

  .content {
    background: white;
    border-radius: 8px;
    padding: 2rem;
    box-shadow: 0 1px 3px rgba(0, 0, 0, 0.1);
  }

  section {
    margin-bottom: 2rem;
  }

  section:last-child {
    margin-bottom: 0;
  }

  h2 {
    color: #374151;
    font-size: 1.5rem;
    margin-bottom: 1rem;
    padding-bottom: 0.5rem;
    border-bottom: 2px solid #e5e7eb;
  }

  h3 {
    color: #4b5563;
    font-size: 1.25rem;
    margin-top: 1.5rem;
    margin-bottom: 0.75rem;
  }

  h4 {
    color: #111827;
    font-size: 1.125rem;
    margin-bottom: 0.5rem;
  }

  p {
    color: #6b7280;
    line-height: 1.6;
    margin-bottom: 1rem;
  }

  ul {
    color: #6b7280;
    line-height: 1.8;
    margin-left: 1.5rem;
    margin-bottom: 1rem;
  }

  li {
    margin-bottom: 0.5rem;
  }

  .code-block {
    background: #f9fafb;
    border: 1px solid #e5e7eb;
    border-radius: 4px;
    padding: 1rem;
    margin: 1rem 0;
    font-family: 'Courier New', monospace;
  }

  .code-block code {
    color: #111827;
    font-size: 0.875rem;
  }

  .info-box {
    background: #eff6ff;
    border-left: 4px solid #3b82f6;
    padding: 1rem 1.5rem;
    margin: 1.5rem 0;
    border-radius: 4px;
  }

  .info-box strong {
    color: #1e40af;
    display: block;
    margin-bottom: 0.75rem;
  }

  .info-box ul {
    margin-left: 1rem;
    margin-bottom: 0;
  }

  .warning-box {
    background: #fef3c7;
    border: 1px solid #fbbf24;
    border-radius: 8px;
    padding: 1.5rem;
  }

  .warning-box h2 {
    color: #92400e;
    border-bottom: none;
    margin-bottom: 1rem;
  }

  .warning-box ul {
    color: #78350f;
    margin-bottom: 0;
  }

  .steps {
    margin-top: 1.5rem;
  }

  .step {
    display: flex;
    gap: 1rem;
    margin-bottom: 1.5rem;
    padding-bottom: 1.5rem;
    border-bottom: 1px solid #e5e7eb;
  }

  .step:last-child {
    border-bottom: none;
    margin-bottom: 0;
    padding-bottom: 0;
  }

  .step-number {
    flex-shrink: 0;
    width: 2.5rem;
    height: 2.5rem;
    background: #3b82f6;
    color: white;
    border-radius: 50%;
    display: flex;
    align-items: center;
    justify-content: center;
    font-weight: 700;
    font-size: 1.125rem;
  }

  .step-content {
    flex: 1;
  }

  .step-content h4 {
    margin-top: 0.25rem;
  }

  .step-content p {
    margin-bottom: 0;
  }

  @media (max-width: 640px) {
    .help-container {
      padding: 1rem 0.5rem;
    }

    .content {
      padding: 1.5rem;
    }

    h1 {
      font-size: 1.5rem;
    }

    h2 {
      font-size: 1.25rem;
    }

    .step {
      flex-direction: column;
      gap: 0.5rem;
    }

    .step-number {
      width: 2rem;
      height: 2rem;
      font-size: 1rem;
    }
  }
</style>
