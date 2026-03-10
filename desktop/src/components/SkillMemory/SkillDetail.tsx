/**
 * SkillDetail Component
 *
 * Full detail view for a skill within the management dialog.
 * Shows name, description, source, tags, body (markdown), and metadata.
 */

import { useEffect, useState } from 'react';
import { clsx } from 'clsx';
import { useTranslation } from 'react-i18next';
import { Cross2Icon } from '@radix-ui/react-icons';
import { SkillSourceBadge } from './SkillSourceBadge';
import type { SkillDocument, SkillReviewStatus } from '../../types/skillMemory';
import { useSkillMemoryStore } from '../../store/skillMemory';

function reviewStatusLabelFallback(status: SkillReviewStatus | null | undefined): string {
  switch (status) {
    case 'approved':
      return 'Approved';
    case 'rejected':
      return 'Rejected';
    case 'archived':
      return 'Archived';
    case 'pending_review':
    default:
      return 'Pending Review';
  }
}

function reviewStatusTone(status: SkillReviewStatus | null | undefined): string {
  switch (status) {
    case 'approved':
      return 'bg-emerald-100 text-emerald-700 dark:bg-emerald-900/20 dark:text-emerald-300';
    case 'rejected':
      return 'bg-red-100 text-red-700 dark:bg-red-900/20 dark:text-red-300';
    case 'archived':
      return 'bg-gray-200 text-gray-700 dark:bg-gray-800 dark:text-gray-300';
    case 'pending_review':
    default:
      return 'bg-amber-100 text-amber-700 dark:bg-amber-900/20 dark:text-amber-300';
  }
}

function toolPolicyModeLabelFallback(mode: SkillDocument['tool_policy_mode']): string {
  return mode === 'restrictive' ? 'Restrictive' : 'Advisory';
}

interface SkillDetailProps {
  skill: SkillDocument;
  onClose: () => void;
  className?: string;
  projectPath?: string | null;
}

export function SkillDetail({ skill, onClose, className, projectPath }: SkillDetailProps) {
  const { t } = useTranslation('simpleMode');
  const updateGeneratedSkill = useSkillMemoryStore((s) => s.updateGeneratedSkill);
  const reviewGeneratedSkill = useSkillMemoryStore((s) => s.reviewGeneratedSkill);
  const deleteSkill = useSkillMemoryStore((s) => s.deleteSkill);
  const exportGeneratedSkill = useSkillMemoryStore((s) => s.exportGeneratedSkill);
  const [isEditing, setIsEditing] = useState(false);
  const [draftName, setDraftName] = useState(skill.name);
  const [draftDescription, setDraftDescription] = useState(skill.description);
  const [draftTags, setDraftTags] = useState(skill.tags.join(', '));
  const [draftBody, setDraftBody] = useState(skill.body);
  const phaseLabel = (phase: string) =>
    t(`skillPanel.phaseLabels.${phase}`, {
      defaultValue: phase,
    });
  const metadataEntries = Object.entries(skill.metadata ?? {});

  useEffect(() => {
    setDraftName(skill.name);
    setDraftDescription(skill.description);
    setDraftTags(skill.tags.join(', '));
    setDraftBody(skill.body);
    setIsEditing(false);
  }, [skill]);

  const generatedSkill = skill.source.type === 'generated';
  const handleExport = async () => {
    const json = await exportGeneratedSkill(skill.id);
    if (!json) return;
    const blob = new Blob([json], { type: 'application/json' });
    const url = URL.createObjectURL(blob);
    const link = document.createElement('a');
    link.href = url;
    link.download = `${skill.name.replace(/[^a-z0-9-_]+/gi, '-').toLowerCase() || 'generated-skill'}.json`;
    link.click();
    URL.revokeObjectURL(url);
  };

  const handleSave = async () => {
    await updateGeneratedSkill(skill.id, {
      name: draftName.trim(),
      description: draftDescription.trim(),
      tags: draftTags
        .split(',')
        .map((tag) => tag.trim())
        .filter(Boolean),
      body: draftBody,
    });
    setIsEditing(false);
  };

  const handleDelete = async () => {
    if (!projectPath) return;
    await deleteSkill(skill.id, projectPath);
    onClose();
  };

  return (
    <div data-testid="skill-detail" className={clsx('flex flex-col h-full', className)}>
      {/* Header */}
      <div className="flex items-start justify-between p-4 border-b border-gray-200 dark:border-gray-700">
        <div className="min-w-0 flex-1">
          <div className="flex items-center gap-2 mb-1">
            <h3 className="text-sm font-semibold text-gray-900 dark:text-white truncate">{skill.name}</h3>
            <SkillSourceBadge source={skill.source} />
            {(skill.source.type === 'generated' || skill.review_status) && (
              <span
                className={clsx(
                  'inline-flex items-center rounded-full px-2 py-0.5 text-xs font-medium',
                  reviewStatusTone(skill.review_status),
                )}
              >
                {t(`skillPanel.reviewStatus.${skill.review_status ?? 'pending_review'}`, {
                  defaultValue: reviewStatusLabelFallback(skill.review_status),
                })}
              </span>
            )}
          </div>
          <p className="text-xs text-gray-500 dark:text-gray-400">{skill.description}</p>
        </div>
        <button
          onClick={onClose}
          className={clsx(
            'p-1 rounded-md shrink-0 ml-2',
            'text-gray-400 hover:text-gray-600 dark:hover:text-gray-300',
            'hover:bg-gray-100 dark:hover:bg-gray-800',
          )}
          title={t('skillPanel.close')}
        >
          <Cross2Icon className="w-4 h-4" />
        </button>
      </div>

      {generatedSkill && (
        <div className="px-4 py-2 border-b border-gray-200 dark:border-gray-700 flex flex-wrap items-center gap-2">
          <button
            onClick={() => setIsEditing((value) => !value)}
            className="rounded-md border border-gray-200 px-2 py-1 text-2xs text-gray-600 hover:bg-gray-50 dark:border-gray-700 dark:text-gray-300 dark:hover:bg-gray-800"
          >
            {isEditing
              ? t('skillPanel.cancelEditGenerated', { defaultValue: 'Cancel Edit' })
              : t('skillPanel.editGenerated', { defaultValue: 'Edit Generated' })}
          </button>
          <button
            onClick={() => void handleExport()}
            className="rounded-md border border-gray-200 px-2 py-1 text-2xs text-gray-600 hover:bg-gray-50 dark:border-gray-700 dark:text-gray-300 dark:hover:bg-gray-800"
          >
            {t('skillPanel.exportGenerated', { defaultValue: 'Export Generated' })}
          </button>
          <button
            onClick={() => void reviewGeneratedSkill(skill.id, 'approved')}
            className="rounded-md border border-emerald-200 px-2 py-1 text-2xs text-emerald-700 hover:bg-emerald-50 dark:border-emerald-800 dark:text-emerald-300 dark:hover:bg-emerald-900/20"
          >
            {t('skillPanel.reviewActions.approve', { defaultValue: 'Approve' })}
          </button>
          <button
            onClick={() => void reviewGeneratedSkill(skill.id, 'rejected')}
            className="rounded-md border border-red-200 px-2 py-1 text-2xs text-red-700 hover:bg-red-50 dark:border-red-800 dark:text-red-300 dark:hover:bg-red-900/20"
          >
            {t('skillPanel.reviewActions.reject', { defaultValue: 'Reject' })}
          </button>
          <button
            onClick={() =>
              void reviewGeneratedSkill(skill.id, skill.review_status === 'archived' ? 'pending_review' : 'archived')
            }
            className="rounded-md border border-gray-200 px-2 py-1 text-2xs text-gray-600 hover:bg-gray-50 dark:border-gray-700 dark:text-gray-300 dark:hover:bg-gray-800"
          >
            {skill.review_status === 'archived'
              ? t('skillPanel.reviewActions.restore', { defaultValue: 'Restore' })
              : t('skillPanel.reviewActions.archive', { defaultValue: 'Archive' })}
          </button>
          {projectPath && (
            <button
              onClick={() => void handleDelete()}
              className="rounded-md border border-red-200 px-2 py-1 text-2xs text-red-700 hover:bg-red-50 dark:border-red-800 dark:text-red-300 dark:hover:bg-red-900/20"
            >
              {t('skillPanel.delete', { defaultValue: 'Delete' })}
            </button>
          )}
        </div>
      )}

      {/* Metadata */}
      <div className="px-4 py-3 border-b border-gray-200 dark:border-gray-700 space-y-2">
        {/* Tags */}
        {skill.tags.length > 0 && (
          <div className="flex items-center gap-1 flex-wrap">
            <span className="text-2xs text-gray-500 dark:text-gray-400 shrink-0">{t('skillPanel.tags')}:</span>
            {skill.tags.map((tag) => (
              <span
                key={tag}
                className="text-2xs px-1.5 py-0.5 rounded-full bg-gray-100 dark:bg-gray-700 text-gray-600 dark:text-gray-300"
              >
                {tag}
              </span>
            ))}
          </div>
        )}

        {/* Version */}
        {skill.version && (
          <div className="flex items-center gap-1">
            <span className="text-2xs text-gray-500 dark:text-gray-400">{t('skillPanel.version')}:</span>
            <span className="text-2xs text-gray-700 dark:text-gray-300">{skill.version}</span>
          </div>
        )}

        {/* Priority */}
        <div className="flex items-center gap-1">
          <span className="text-2xs text-gray-500 dark:text-gray-400">{t('skillPanel.priority')}:</span>
          <span className="text-2xs text-gray-700 dark:text-gray-300">{skill.priority}</span>
        </div>

        <div className="flex items-center gap-1 flex-wrap">
          <span className="text-2xs text-gray-500 dark:text-gray-400">
            {t('skillPanel.toolPolicy', { defaultValue: 'Tool policy' })}:
          </span>
          <span
            className={clsx(
              'inline-flex items-center rounded-full px-1.5 py-0.5 text-2xs font-medium',
              skill.tool_policy_mode === 'restrictive'
                ? 'bg-amber-100 text-amber-700 dark:bg-amber-900/20 dark:text-amber-300'
                : 'bg-slate-100 text-slate-700 dark:bg-slate-800 dark:text-slate-300',
            )}
          >
            {t(`skillPanel.toolPolicyModes.${skill.tool_policy_mode}`, {
              defaultValue: toolPolicyModeLabelFallback(skill.tool_policy_mode),
            })}
          </span>
          <span className="text-2xs text-gray-500 dark:text-gray-400">
            {skill.tool_policy_mode === 'restrictive'
              ? t('skillPanel.toolPolicyRestrictiveHint', {
                  defaultValue: 'This skill can hard-restrict runtime tools.',
                })
              : t('skillPanel.toolPolicyAdvisoryHint', {
                  defaultValue: 'This skill provides guidance only and does not hard-restrict runtime tools.',
                })}
          </span>
        </div>

        {skill.tool_policy_mode === 'restrictive' && skill.allowed_tools.length > 0 && (
          <div className="flex items-start gap-1">
            <span className="text-2xs text-gray-500 dark:text-gray-400">
              {t('skillPanel.allowedTools', { defaultValue: 'Allowed tools' })}:
            </span>
            <span className="text-2xs text-gray-700 dark:text-gray-300">{skill.allowed_tools.join(', ')}</span>
          </div>
        )}

        {skill.review_notes && (
          <div className="flex items-start gap-1">
            <span className="text-2xs text-gray-500 dark:text-gray-400">
              {t('skillPanel.reviewNotes', { defaultValue: 'Review notes' })}:
            </span>
            <span className="text-2xs text-gray-700 dark:text-gray-300">{skill.review_notes}</span>
          </div>
        )}

        {/* Injection phases */}
        <div className="flex items-center gap-1 flex-wrap">
          <span className="text-2xs text-gray-500 dark:text-gray-400 shrink-0">{t('skillPanel.phases')}:</span>
          {skill.inject_into.map((phase) => (
            <span
              key={phase}
              className="text-2xs px-1.5 py-0.5 rounded bg-blue-50 dark:bg-blue-900/20 text-blue-600 dark:text-blue-400"
            >
              {phaseLabel(phase)}
            </span>
          ))}
        </div>
      </div>

      {/* Body */}
      <div className="flex-1 overflow-y-auto p-4">
        {isEditing ? (
          <div className="space-y-3">
            <input
              value={draftName}
              onChange={(event) => setDraftName(event.target.value)}
              className="w-full rounded-md border border-gray-200 bg-white px-3 py-2 text-xs text-gray-700 dark:border-gray-700 dark:bg-gray-900 dark:text-gray-200"
              placeholder={t('skillPanel.generatedForm.name', { defaultValue: 'Generated skill name' })}
            />
            <input
              value={draftDescription}
              onChange={(event) => setDraftDescription(event.target.value)}
              className="w-full rounded-md border border-gray-200 bg-white px-3 py-2 text-xs text-gray-700 dark:border-gray-700 dark:bg-gray-900 dark:text-gray-200"
              placeholder={t('skillPanel.generatedForm.description', { defaultValue: 'Description' })}
            />
            <input
              value={draftTags}
              onChange={(event) => setDraftTags(event.target.value)}
              className="w-full rounded-md border border-gray-200 bg-white px-3 py-2 text-xs text-gray-700 dark:border-gray-700 dark:bg-gray-900 dark:text-gray-200"
              placeholder={t('skillPanel.generatedForm.tags', { defaultValue: 'Comma-separated tags' })}
            />
            <textarea
              value={draftBody}
              onChange={(event) => setDraftBody(event.target.value)}
              className="min-h-[280px] w-full rounded-md border border-gray-200 bg-white px-3 py-2 text-xs font-mono text-gray-700 dark:border-gray-700 dark:bg-gray-900 dark:text-gray-200"
            />
            <div className="flex justify-end">
              <button
                onClick={() => void handleSave()}
                className="rounded-md bg-primary-600 px-3 py-1.5 text-xs font-medium text-white hover:bg-primary-700"
              >
                {t('skillPanel.save', { defaultValue: 'Save' })}
              </button>
            </div>
          </div>
        ) : (
          <pre className="text-xs text-gray-700 dark:text-gray-300 whitespace-pre-wrap font-mono leading-relaxed">
            {skill.body}
          </pre>
        )}

        {metadataEntries.length > 0 && !isEditing && (
          <div className="mt-4 border-t border-gray-200 pt-3 dark:border-gray-700">
            <p className="mb-2 text-2xs font-semibold uppercase tracking-wide text-gray-500 dark:text-gray-400">
              {t('skillPanel.metadata', { defaultValue: 'Metadata' })}
            </p>
            <div className="space-y-1">
              {metadataEntries.map(([key, value]) => (
                <div key={key} className="flex items-start gap-2 text-2xs text-gray-500 dark:text-gray-400">
                  <span className="min-w-24 shrink-0">{key}</span>
                  <span className="text-gray-700 dark:text-gray-300">{value}</span>
                </div>
              ))}
            </div>
          </div>
        )}
      </div>
    </div>
  );
}

export default SkillDetail;
