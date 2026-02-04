import { describe, it, expect } from 'vitest';
import { readFileSync, existsSync } from 'fs';
import { join } from 'path';

describe('Navigation - Project Routes', () => {
  const projectRoutesDir = join(__dirname, '../routes/projects/[id]');

  // Define expected project sub-routes based on the navigation
  const expectedRoutes = ['overview', 'transactions', 'history', 'settings'];

  it('should have all navigation routes as actual page files', () => {
    for (const route of expectedRoutes) {
      const routePath = join(projectRoutesDir, route, '+page.svelte');
      expect(
        existsSync(routePath),
        `Route ${route} should have a +page.svelte file at ${routePath}`
      ).toBe(true);
    }
  });

  it('should redirect to an existing route from project page', () => {
    const redirectFile = join(projectRoutesDir, '+page.ts');
    expect(existsSync(redirectFile), 'Project +page.ts should exist').toBe(true);

    const content = readFileSync(redirectFile, 'utf-8');

    // Check that the redirect points to one of the expected routes
    const redirectMatch = content.match(
      /redirect\([^,]+,\s*`\/projects\/\$\{params\.id\}\/([^`]+)`\)/
    );
    expect(redirectMatch, 'Should have a redirect call').toBeTruthy();

    if (redirectMatch) {
      const redirectTarget = redirectMatch[1];
      expect(
        expectedRoutes.includes(redirectTarget),
        `Redirect target "${redirectTarget}" should be one of the expected routes: ${expectedRoutes.join(', ')}`
      ).toBe(true);
    }
  });

  it('should have all navigation links in layout pointing to existing routes', () => {
    const layoutFile = join(projectRoutesDir, '+layout.svelte');
    expect(existsSync(layoutFile), 'Project +layout.svelte should exist').toBe(true);

    const content = readFileSync(layoutFile, 'utf-8');

    // Extract all href patterns from the navigation
    const hrefMatches = content.matchAll(/href=\{resolve\(`\/projects\/\$\{[^}]+\}\/([^`]+)`\)\}/g);

    const routesInLayout: string[] = [];
    for (const match of hrefMatches) {
      routesInLayout.push(match[1]);
    }

    expect(routesInLayout.length, 'Should have navigation links in layout').toBeGreaterThan(0);

    // Check each route in the layout exists as a page
    for (const route of routesInLayout) {
      // Settings is admin-only but should still exist
      const routePath = join(projectRoutesDir, route, '+page.svelte');
      expect(
        existsSync(routePath),
        `Navigation link to "${route}" should have a corresponding page file at ${routePath}`
      ).toBe(true);
    }
  });

  it('should have navigation routes match expected routes', () => {
    const layoutFile = join(projectRoutesDir, '+layout.svelte');
    const content = readFileSync(layoutFile, 'utf-8');

    const hrefMatches = content.matchAll(/href=\{resolve\(`\/projects\/\$\{[^}]+\}\/([^`]+)`\)\}/g);

    const routesInLayout = Array.from(hrefMatches).map((match) => match[1]);

    // All routes in layout should be in expected routes
    for (const route of routesInLayout) {
      expect(
        expectedRoutes.includes(route),
        `Route "${route}" in layout should be in expected routes list`
      ).toBe(true);
    }

    // All expected routes (except settings which is conditional) should be in layout
    const requiredRoutes = expectedRoutes.filter((r) => r !== 'settings');
    for (const route of requiredRoutes) {
      expect(
        routesInLayout.includes(route),
        `Expected route "${route}" should be in layout navigation`
      ).toBe(true);
    }
  });

  it('should not have references to deprecated routes', () => {
    const layoutFile = join(projectRoutesDir, '+layout.svelte');
    const redirectFile = join(projectRoutesDir, '+page.ts');

    const layoutContent = readFileSync(layoutFile, 'utf-8');
    const redirectContent = readFileSync(redirectFile, 'utf-8');

    const deprecatedRoutes = ['debts', 'participants', 'members', 'payments', 'cashflow']; // Add more deprecated routes here if needed

    for (const route of deprecatedRoutes) {
      expect(
        layoutContent.includes(`/${route}`),
        `Layout should not reference deprecated route: ${route}`
      ).toBe(false);

      expect(
        redirectContent.includes(`/${route}`),
        `Redirect should not reference deprecated route: ${route}`
      ).toBe(false);
    }
  });
});
