import { writable, derived, get } from 'svelte/store';
import type { Project, ProjectWithRole, Participant, ProjectMember } from '$lib/api';
import * as api from '$lib/api';

// Current project
export const currentProject = writable<ProjectWithRole | null>(null);
export const participants = writable<Participant[]>([]);
export const members = writable<ProjectMember[]>([]);

// Derived stores
export const isAdmin = derived(currentProject, ($project) => $project?.role === 'admin');
export const isEditor = derived(currentProject, ($project) =>
    $project?.role === 'admin' || $project?.role === 'editor'
);
export const canEdit = isEditor;

// Helper to load project data
export async function loadProject(projectId: number) {
    const [project, participantsList, membersList] = await Promise.all([
        api.getProject(projectId),
        api.getParticipants(projectId),
        api.getMembers(projectId)
    ]);

    // Get role from members list for current user
    const projectsWithRole = await api.getProjects();
    const projectWithRole = projectsWithRole.find(p => p.id === projectId);

    currentProject.set(projectWithRole || { ...project, role: 'reader', owner_name: '', user_balance: null, user_pools: [] });
    participants.set(participantsList);
    members.set(membersList);
}

export function clearProject() {
    currentProject.set(null);
    participants.set([]);
    members.set([]);
}

// Helper to refresh participants
export async function refreshParticipants(projectId: number) {
    const participantsList = await api.getParticipants(projectId);
    participants.set(participantsList);
}

// Helper to refresh members
export async function refreshMembers(projectId: number) {
    const membersList = await api.getMembers(projectId);
    members.set(membersList);
}
