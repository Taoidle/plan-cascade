/**
 * DesignDocPanel Component
 *
 * Viewer and editor for design documents in Expert Mode.
 * Displays architecture overview, components, APIs, ADRs,
 * and story-to-component mappings with collapsible sections.
 * Supports generation from PRD and import of external documents.
 */

import { useState } from 'react';
import { clsx } from 'clsx';
import { useDesignDocStore } from '../../store/designDoc';
import type {
  DesignDoc,
  DesignComponent,
  DesignPattern,
  DesignDecision,
  FeatureMapping,
  ImportWarning,
  GenerationInfo,
} from '../../store/designDoc';

// ============================================================================
// Main Panel
// ============================================================================

export function DesignDocPanel() {
  const {
    designDoc,
    generationInfo,
    importWarnings,
    loading,
    error,
    generateDesignDoc,
    importDesignDoc,
    loadDesignDoc,
    reset,
    clearError,
  } = useDesignDocStore();

  // If no document loaded, show action panel
  if (!designDoc) {
    return (
      <DesignDocActions
        onGenerate={generateDesignDoc}
        onImport={importDesignDoc}
        onLoad={loadDesignDoc}
        loading={loading}
        error={error}
        onClearError={clearError}
      />
    );
  }

  // Document loaded - show viewer
  return (
    <div className="h-full flex flex-col">
      {/* Header */}
      <DesignDocHeader
        doc={designDoc}
        generationInfo={generationInfo}
        onReset={reset}
      />

      {/* Warnings Banner */}
      {importWarnings && importWarnings.length > 0 && (
        <WarningsBanner warnings={importWarnings} />
      )}

      {/* Error Banner */}
      {error && (
        <div className="mx-6 mt-2 px-4 py-2 bg-red-50 dark:bg-red-900/20 border border-red-200 dark:border-red-800 rounded-lg">
          <div className="flex items-center justify-between">
            <span className="text-sm text-red-700 dark:text-red-300">{error}</span>
            <button
              onClick={clearError}
              className="text-red-500 hover:text-red-700 text-sm font-medium"
            >
              Dismiss
            </button>
          </div>
        </div>
      )}

      {/* Content */}
      <div className="flex-1 overflow-auto p-6 space-y-4">
        <OverviewSection doc={designDoc} />
        <ArchitectureSection doc={designDoc} />
        <ComponentsSection components={designDoc.architecture.components} />
        <PatternsSection patterns={designDoc.architecture.patterns} />
        <InterfacesSection doc={designDoc} />
        <DecisionsSection decisions={designDoc.decisions} />
        <FeatureMappingsSection mappings={designDoc.feature_mappings} />
      </div>
    </div>
  );
}

// ============================================================================
// Actions Panel (no document loaded)
// ============================================================================

interface DesignDocActionsProps {
  onGenerate: (prdPath: string) => Promise<unknown>;
  onImport: (filePath: string, format?: string) => Promise<unknown>;
  onLoad: (projectPath?: string) => Promise<unknown>;
  loading: { generating: boolean; importing: boolean; loading: boolean };
  error: string | null;
  onClearError: () => void;
}

function DesignDocActions({
  onGenerate,
  onImport,
  onLoad,
  loading,
  error,
  onClearError,
}: DesignDocActionsProps) {
  const [prdPath, setPrdPath] = useState('');
  const [importPath, setImportPath] = useState('');
  const [importFormat, setImportFormat] = useState<string>('');
  const [loadPath, setLoadPath] = useState('');
  const [activeAction, setActiveAction] = useState<'generate' | 'import' | 'load'>('generate');

  const handleGenerate = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!prdPath.trim()) return;
    await onGenerate(prdPath.trim());
  };

  const handleImport = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!importPath.trim()) return;
    await onImport(importPath.trim(), importFormat || undefined);
  };

  const handleLoad = async (e: React.FormEvent) => {
    e.preventDefault();
    await onLoad(loadPath.trim() || undefined);
  };

  return (
    <div className="h-full flex items-center justify-center p-6">
      <div className="max-w-lg w-full">
        <div className="mb-8 text-center">
          <h2 className="text-2xl font-semibold text-gray-900 dark:text-white mb-2">
            Design Document
          </h2>
          <p className="text-gray-600 dark:text-gray-400">
            Generate a design document from a PRD, import an existing document,
            or load one from your project.
          </p>
        </div>

        {error && (
          <div className="mb-4 px-4 py-2 bg-red-50 dark:bg-red-900/20 border border-red-200 dark:border-red-800 rounded-lg">
            <div className="flex items-center justify-between">
              <span className="text-sm text-red-700 dark:text-red-300">{error}</span>
              <button onClick={onClearError} className="text-red-500 hover:text-red-700 text-sm">
                Dismiss
              </button>
            </div>
          </div>
        )}

        {/* Action Tabs */}
        <div className="flex gap-1 mb-4 border-b border-gray-200 dark:border-gray-700">
          {([
            { id: 'generate' as const, label: 'Generate from PRD' },
            { id: 'import' as const, label: 'Import' },
            { id: 'load' as const, label: 'Load Existing' },
          ]).map((tab) => (
            <button
              key={tab.id}
              onClick={() => setActiveAction(tab.id)}
              className={clsx(
                'px-4 py-2 rounded-t-lg text-sm font-medium transition-colors',
                activeAction === tab.id
                  ? 'bg-gray-100 dark:bg-gray-800 text-primary-600 dark:text-primary-400'
                  : 'text-gray-500 dark:text-gray-400 hover:text-gray-700 dark:hover:text-gray-200'
              )}
            >
              {tab.label}
            </button>
          ))}
        </div>

        {/* Generate Form */}
        {activeAction === 'generate' && (
          <form onSubmit={handleGenerate} className="space-y-4">
            <div>
              <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">
                PRD File Path
              </label>
              <input
                type="text"
                value={prdPath}
                onChange={(e) => setPrdPath(e.target.value)}
                placeholder="/path/to/prd.json"
                className={clsx(
                  'w-full px-4 py-2.5 rounded-lg border text-sm',
                  'bg-white dark:bg-gray-800',
                  'border-gray-300 dark:border-gray-600',
                  'text-gray-900 dark:text-white',
                  'placeholder-gray-400 dark:placeholder-gray-500',
                  'focus:ring-2 focus:ring-primary-500 focus:border-primary-500'
                )}
                required
              />
              <p className="mt-1 text-xs text-gray-500 dark:text-gray-400">
                Path to the prd.json file generated from a spec interview or PRD generation.
              </p>
            </div>
            <button
              type="submit"
              disabled={loading.generating || !prdPath.trim()}
              className={clsx(
                'w-full px-6 py-3 rounded-lg font-medium text-white',
                'bg-primary-600 hover:bg-primary-700',
                'disabled:opacity-50 disabled:cursor-not-allowed',
                'transition-colors'
              )}
            >
              {loading.generating ? 'Generating...' : 'Generate Design Document'}
            </button>
          </form>
        )}

        {/* Import Form */}
        {activeAction === 'import' && (
          <form onSubmit={handleImport} className="space-y-4">
            <div>
              <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">
                File Path
              </label>
              <input
                type="text"
                value={importPath}
                onChange={(e) => setImportPath(e.target.value)}
                placeholder="/path/to/design.md or /path/to/design.json"
                className={clsx(
                  'w-full px-4 py-2.5 rounded-lg border text-sm',
                  'bg-white dark:bg-gray-800',
                  'border-gray-300 dark:border-gray-600',
                  'text-gray-900 dark:text-white',
                  'placeholder-gray-400 dark:placeholder-gray-500',
                  'focus:ring-2 focus:ring-primary-500 focus:border-primary-500'
                )}
                required
              />
            </div>
            <div>
              <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">
                Format (optional)
              </label>
              <div className="flex gap-3">
                {(['', 'markdown', 'json'] as const).map((fmt) => (
                  <button
                    key={fmt || 'auto'}
                    type="button"
                    onClick={() => setImportFormat(fmt)}
                    className={clsx(
                      'flex-1 px-4 py-2 rounded-lg text-sm font-medium transition-colors',
                      importFormat === fmt
                        ? 'bg-primary-600 text-white'
                        : 'bg-gray-100 dark:bg-gray-700 text-gray-700 dark:text-gray-300 hover:bg-gray-200 dark:hover:bg-gray-600'
                    )}
                  >
                    {fmt || 'Auto-detect'}
                  </button>
                ))}
              </div>
            </div>
            <button
              type="submit"
              disabled={loading.importing || !importPath.trim()}
              className={clsx(
                'w-full px-6 py-3 rounded-lg font-medium text-white',
                'bg-primary-600 hover:bg-primary-700',
                'disabled:opacity-50 disabled:cursor-not-allowed',
                'transition-colors'
              )}
            >
              {loading.importing ? 'Importing...' : 'Import Design Document'}
            </button>
          </form>
        )}

        {/* Load Form */}
        {activeAction === 'load' && (
          <form onSubmit={handleLoad} className="space-y-4">
            <div>
              <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">
                Project Path (optional)
              </label>
              <input
                type="text"
                value={loadPath}
                onChange={(e) => setLoadPath(e.target.value)}
                placeholder="Leave empty for current directory"
                className={clsx(
                  'w-full px-4 py-2.5 rounded-lg border text-sm',
                  'bg-white dark:bg-gray-800',
                  'border-gray-300 dark:border-gray-600',
                  'text-gray-900 dark:text-white',
                  'placeholder-gray-400 dark:placeholder-gray-500',
                  'focus:ring-2 focus:ring-primary-500 focus:border-primary-500'
                )}
              />
              <p className="mt-1 text-xs text-gray-500 dark:text-gray-400">
                Loads design_doc.json from the project root directory.
              </p>
            </div>
            <button
              type="submit"
              disabled={loading.loading}
              className={clsx(
                'w-full px-6 py-3 rounded-lg font-medium text-white',
                'bg-primary-600 hover:bg-primary-700',
                'disabled:opacity-50 disabled:cursor-not-allowed',
                'transition-colors'
              )}
            >
              {loading.loading ? 'Loading...' : 'Load Design Document'}
            </button>
          </form>
        )}
      </div>
    </div>
  );
}

// ============================================================================
// Header
// ============================================================================

interface DesignDocHeaderProps {
  doc: DesignDoc;
  generationInfo: GenerationInfo | null;
  onReset: () => void;
}

function DesignDocHeader({ doc, generationInfo, onReset }: DesignDocHeaderProps) {
  return (
    <div className="px-6 py-4 border-b border-gray-200 dark:border-gray-700 flex items-center justify-between">
      <div>
        <h2 className="text-lg font-semibold text-gray-900 dark:text-white">
          {doc.overview.title || 'Design Document'}
        </h2>
        <div className="flex items-center gap-3 mt-1">
          <span className={clsx(
            'inline-flex items-center px-2 py-0.5 rounded text-xs font-medium',
            doc.metadata.level === 'project'
              ? 'bg-blue-100 dark:bg-blue-900/30 text-blue-700 dark:text-blue-300'
              : 'bg-purple-100 dark:bg-purple-900/30 text-purple-700 dark:text-purple-300'
          )}>
            {doc.metadata.level}
          </span>
          {doc.metadata.source && (
            <span className="text-xs text-gray-500 dark:text-gray-400">
              Source: {doc.metadata.source}
            </span>
          )}
          {generationInfo && (
            <span className="text-xs text-gray-500 dark:text-gray-400">
              {generationInfo.components_generated} components, {generationInfo.decisions_created} ADRs, {generationInfo.feature_mappings_created} mappings
            </span>
          )}
        </div>
      </div>
      <button
        onClick={onReset}
        className={clsx(
          'px-4 py-2 rounded-lg text-sm font-medium',
          'bg-gray-100 dark:bg-gray-700',
          'text-gray-700 dark:text-gray-300',
          'hover:bg-gray-200 dark:hover:bg-gray-600',
          'transition-colors'
        )}
      >
        New Document
      </button>
    </div>
  );
}

// ============================================================================
// Warnings Banner
// ============================================================================

interface WarningsBannerProps {
  warnings: ImportWarning[];
}

function WarningsBanner({ warnings }: WarningsBannerProps) {
  const [expanded, setExpanded] = useState(false);

  const highCount = warnings.filter((w) => w.severity === 'high').length;
  const mediumCount = warnings.filter((w) => w.severity === 'medium').length;

  return (
    <div className="mx-6 mt-2 px-4 py-2 bg-yellow-50 dark:bg-yellow-900/20 border border-yellow-200 dark:border-yellow-800 rounded-lg">
      <div className="flex items-center justify-between">
        <span className="text-sm text-yellow-700 dark:text-yellow-300">
          {warnings.length} import warning{warnings.length !== 1 ? 's' : ''}
          {highCount > 0 && ` (${highCount} high)`}
          {mediumCount > 0 && ` (${mediumCount} medium)`}
        </span>
        <button
          onClick={() => setExpanded(!expanded)}
          className="text-yellow-600 hover:text-yellow-800 text-sm font-medium"
        >
          {expanded ? 'Hide' : 'Show'}
        </button>
      </div>
      {expanded && (
        <ul className="mt-2 space-y-1">
          {warnings.map((w, idx) => (
            <li key={idx} className="text-xs text-yellow-600 dark:text-yellow-400 flex items-start gap-2">
              <span className={clsx(
                'inline-block px-1.5 py-0.5 rounded text-[10px] font-medium uppercase',
                w.severity === 'high' ? 'bg-red-100 text-red-700' :
                w.severity === 'medium' ? 'bg-yellow-100 text-yellow-700' :
                'bg-gray-100 text-gray-600'
              )}>
                {w.severity}
              </span>
              <span>{w.message}{w.field ? ` (${w.field})` : ''}</span>
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}

// ============================================================================
// Collapsible Section Wrapper
// ============================================================================

interface CollapsibleSectionProps {
  title: string;
  count?: number;
  defaultOpen?: boolean;
  children: React.ReactNode;
}

function CollapsibleSection({ title, count, defaultOpen = true, children }: CollapsibleSectionProps) {
  const [isOpen, setIsOpen] = useState(defaultOpen);

  return (
    <div className="border border-gray-200 dark:border-gray-700 rounded-lg overflow-hidden">
      <button
        onClick={() => setIsOpen(!isOpen)}
        className={clsx(
          'w-full flex items-center justify-between px-4 py-3',
          'bg-gray-50 dark:bg-gray-800',
          'hover:bg-gray-100 dark:hover:bg-gray-750',
          'transition-colors'
        )}
      >
        <div className="flex items-center gap-2">
          <span className={clsx(
            'text-sm transition-transform',
            isOpen ? 'rotate-90' : ''
          )}>
            &rsaquo;
          </span>
          <span className="text-sm font-semibold text-gray-900 dark:text-white">
            {title}
          </span>
          {count !== undefined && (
            <span className="text-xs text-gray-500 dark:text-gray-400 bg-gray-200 dark:bg-gray-700 px-1.5 py-0.5 rounded">
              {count}
            </span>
          )}
        </div>
      </button>
      {isOpen && (
        <div className="p-4 border-t border-gray-200 dark:border-gray-700">
          {children}
        </div>
      )}
    </div>
  );
}

// ============================================================================
// Section Components
// ============================================================================

function OverviewSection({ doc }: { doc: DesignDoc }) {
  return (
    <CollapsibleSection title="Overview">
      <div className="space-y-3">
        {doc.overview.summary && (
          <p className="text-sm text-gray-700 dark:text-gray-300">{doc.overview.summary}</p>
        )}
        {doc.overview.goals.length > 0 && (
          <div>
            <h4 className="text-xs font-semibold text-gray-500 dark:text-gray-400 uppercase tracking-wider mb-1">Goals</h4>
            <ul className="list-disc list-inside text-sm text-gray-700 dark:text-gray-300 space-y-0.5">
              {doc.overview.goals.map((goal, idx) => (
                <li key={idx}>{goal}</li>
              ))}
            </ul>
          </div>
        )}
        {doc.overview.non_goals.length > 0 && (
          <div>
            <h4 className="text-xs font-semibold text-gray-500 dark:text-gray-400 uppercase tracking-wider mb-1">Non-Goals</h4>
            <ul className="list-disc list-inside text-sm text-gray-500 dark:text-gray-400 space-y-0.5">
              {doc.overview.non_goals.map((ng, idx) => (
                <li key={idx}>{ng}</li>
              ))}
            </ul>
          </div>
        )}
      </div>
    </CollapsibleSection>
  );
}

function ArchitectureSection({ doc }: { doc: DesignDoc }) {
  return (
    <CollapsibleSection title="Architecture">
      <div className="space-y-3">
        {doc.architecture.system_overview && (
          <div>
            <h4 className="text-xs font-semibold text-gray-500 dark:text-gray-400 uppercase tracking-wider mb-1">System Overview</h4>
            <p className="text-sm text-gray-700 dark:text-gray-300 whitespace-pre-wrap">{doc.architecture.system_overview}</p>
          </div>
        )}
        {doc.architecture.data_flow && (
          <div>
            <h4 className="text-xs font-semibold text-gray-500 dark:text-gray-400 uppercase tracking-wider mb-1">Data Flow</h4>
            <p className="text-sm text-gray-700 dark:text-gray-300">{doc.architecture.data_flow}</p>
          </div>
        )}
        {(doc.architecture.infrastructure.existing_services.length > 0 ||
          doc.architecture.infrastructure.new_services.length > 0) && (
          <div>
            <h4 className="text-xs font-semibold text-gray-500 dark:text-gray-400 uppercase tracking-wider mb-1">Infrastructure</h4>
            {doc.architecture.infrastructure.new_services.length > 0 && (
              <div className="flex flex-wrap gap-1.5 mt-1">
                {doc.architecture.infrastructure.new_services.map((svc, idx) => (
                  <span
                    key={idx}
                    className="inline-flex items-center px-2 py-0.5 rounded text-xs bg-green-100 dark:bg-green-900/30 text-green-700 dark:text-green-300"
                  >
                    {svc}
                  </span>
                ))}
              </div>
            )}
          </div>
        )}
      </div>
    </CollapsibleSection>
  );
}

function ComponentsSection({ components }: { components: DesignComponent[] }) {
  if (components.length === 0) return null;

  return (
    <CollapsibleSection title="Components" count={components.length}>
      <div className="space-y-3">
        {components.map((comp) => (
          <div
            key={comp.name}
            className="p-3 border border-gray-100 dark:border-gray-700 rounded-lg bg-white dark:bg-gray-800"
          >
            <div className="flex items-start justify-between">
              <div>
                <h4 className="text-sm font-semibold text-gray-900 dark:text-white">{comp.name}</h4>
                {comp.description && (
                  <p className="text-xs text-gray-600 dark:text-gray-400 mt-0.5">{comp.description}</p>
                )}
              </div>
              {comp.features.length > 0 && (
                <div className="flex flex-wrap gap-1">
                  {comp.features.map((feat, idx) => (
                    <span
                      key={idx}
                      className="text-[10px] px-1.5 py-0.5 rounded bg-primary-100 dark:bg-primary-900/30 text-primary-700 dark:text-primary-300"
                    >
                      {feat}
                    </span>
                  ))}
                </div>
              )}
            </div>
            {comp.responsibilities.length > 0 && (
              <ul className="mt-2 text-xs text-gray-600 dark:text-gray-400 space-y-0.5">
                {comp.responsibilities.map((r, idx) => (
                  <li key={idx} className="flex items-start gap-1">
                    <span className="text-gray-400 mt-0.5">-</span>
                    <span>{r}</span>
                  </li>
                ))}
              </ul>
            )}
            {comp.dependencies.length > 0 && (
              <div className="mt-2 flex items-center gap-1">
                <span className="text-[10px] text-gray-400">Deps:</span>
                {comp.dependencies.map((dep, idx) => (
                  <span
                    key={idx}
                    className="text-[10px] px-1.5 py-0.5 rounded bg-gray-100 dark:bg-gray-700 text-gray-600 dark:text-gray-400"
                  >
                    {dep}
                  </span>
                ))}
              </div>
            )}
          </div>
        ))}
      </div>
    </CollapsibleSection>
  );
}

function PatternsSection({ patterns }: { patterns: DesignPattern[] }) {
  if (patterns.length === 0) return null;

  return (
    <CollapsibleSection title="Design Patterns" count={patterns.length} defaultOpen={false}>
      <div className="space-y-2">
        {patterns.map((pattern) => (
          <div
            key={pattern.name}
            className="p-3 border border-gray-100 dark:border-gray-700 rounded-lg bg-white dark:bg-gray-800"
          >
            <h4 className="text-sm font-semibold text-gray-900 dark:text-white">{pattern.name}</h4>
            {pattern.description && (
              <p className="text-xs text-gray-600 dark:text-gray-400 mt-0.5">{pattern.description}</p>
            )}
            {pattern.rationale && (
              <p className="text-xs text-gray-500 dark:text-gray-500 mt-1 italic">
                Rationale: {pattern.rationale}
              </p>
            )}
            {pattern.applies_to.length > 0 && (
              <div className="mt-1 flex flex-wrap gap-1">
                {pattern.applies_to.map((target, idx) => (
                  <span
                    key={idx}
                    className="text-[10px] px-1.5 py-0.5 rounded bg-gray-100 dark:bg-gray-700 text-gray-600 dark:text-gray-400"
                  >
                    {target}
                  </span>
                ))}
              </div>
            )}
          </div>
        ))}
      </div>
    </CollapsibleSection>
  );
}

function InterfacesSection({ doc }: { doc: DesignDoc }) {
  const { api_standards, shared_data_models } = doc.interfaces;
  const hasContent = api_standards.style || shared_data_models.length > 0;

  if (!hasContent) return null;

  return (
    <CollapsibleSection title="API / Interfaces" defaultOpen={false}>
      <div className="space-y-3">
        <div className="grid grid-cols-3 gap-3">
          {api_standards.style && (
            <div>
              <h4 className="text-xs font-semibold text-gray-500 dark:text-gray-400 uppercase tracking-wider mb-0.5">Style</h4>
              <p className="text-sm text-gray-700 dark:text-gray-300">{api_standards.style}</p>
            </div>
          )}
          {api_standards.error_handling && (
            <div>
              <h4 className="text-xs font-semibold text-gray-500 dark:text-gray-400 uppercase tracking-wider mb-0.5">Error Handling</h4>
              <p className="text-sm text-gray-700 dark:text-gray-300">{api_standards.error_handling}</p>
            </div>
          )}
          {api_standards.async_pattern && (
            <div>
              <h4 className="text-xs font-semibold text-gray-500 dark:text-gray-400 uppercase tracking-wider mb-0.5">Async Pattern</h4>
              <p className="text-sm text-gray-700 dark:text-gray-300">{api_standards.async_pattern}</p>
            </div>
          )}
        </div>
      </div>
    </CollapsibleSection>
  );
}

function DecisionsSection({ decisions }: { decisions: DesignDecision[] }) {
  if (decisions.length === 0) return null;

  const statusColors: Record<string, string> = {
    proposed: 'bg-yellow-100 dark:bg-yellow-900/30 text-yellow-700 dark:text-yellow-300',
    accepted: 'bg-green-100 dark:bg-green-900/30 text-green-700 dark:text-green-300',
    deprecated: 'bg-red-100 dark:bg-red-900/30 text-red-700 dark:text-red-300',
    superseded: 'bg-gray-100 dark:bg-gray-700 text-gray-600 dark:text-gray-400',
  };

  return (
    <CollapsibleSection title="Architecture Decisions (ADRs)" count={decisions.length} defaultOpen={false}>
      <div className="space-y-2">
        {decisions.map((decision) => (
          <div
            key={decision.id}
            className="p-3 border border-gray-100 dark:border-gray-700 rounded-lg bg-white dark:bg-gray-800"
          >
            <div className="flex items-center gap-2">
              <span className="text-xs font-mono text-gray-500 dark:text-gray-400">{decision.id}</span>
              <h4 className="text-sm font-semibold text-gray-900 dark:text-white flex-1">{decision.title}</h4>
              <span className={clsx(
                'text-[10px] px-1.5 py-0.5 rounded font-medium',
                statusColors[decision.status] || statusColors.proposed
              )}>
                {decision.status}
              </span>
            </div>
            {decision.context && (
              <p className="text-xs text-gray-600 dark:text-gray-400 mt-1">{decision.context}</p>
            )}
            {decision.rationale && (
              <p className="text-xs text-gray-500 dark:text-gray-500 mt-1 italic">
                Rationale: {decision.rationale}
              </p>
            )}
            {decision.applies_to.length > 0 && (
              <div className="mt-1 flex flex-wrap gap-1">
                <span className="text-[10px] text-gray-400">Applies to:</span>
                {decision.applies_to.map((target, idx) => (
                  <span
                    key={idx}
                    className="text-[10px] px-1.5 py-0.5 rounded bg-gray-100 dark:bg-gray-700 text-gray-600 dark:text-gray-400"
                  >
                    {target}
                  </span>
                ))}
              </div>
            )}
          </div>
        ))}
      </div>
    </CollapsibleSection>
  );
}

function FeatureMappingsSection({ mappings }: { mappings: Record<string, FeatureMapping> }) {
  const entries = Object.entries(mappings);
  if (entries.length === 0) return null;

  return (
    <CollapsibleSection title="Story-to-Component Mappings" count={entries.length}>
      <div className="space-y-3">
        {entries.map(([featureId, mapping]) => (
          <div
            key={featureId}
            className="p-3 border border-gray-100 dark:border-gray-700 rounded-lg bg-white dark:bg-gray-800"
          >
            <div className="flex items-center gap-2 mb-2">
              <span className="text-sm font-semibold text-primary-600 dark:text-primary-400 font-mono">
                {featureId}
              </span>
              {mapping.description && (
                <span className="text-xs text-gray-500 dark:text-gray-400">
                  - {mapping.description}
                </span>
              )}
            </div>

            {/* Visual mapping display */}
            <div className="flex flex-wrap gap-4">
              {mapping.components.length > 0 && (
                <div>
                  <span className="text-[10px] font-semibold text-gray-400 uppercase tracking-wider">Components</span>
                  <div className="flex flex-wrap gap-1 mt-0.5">
                    {mapping.components.map((comp, idx) => (
                      <span
                        key={idx}
                        className="text-xs px-2 py-0.5 rounded-full bg-blue-100 dark:bg-blue-900/30 text-blue-700 dark:text-blue-300"
                      >
                        {comp}
                      </span>
                    ))}
                  </div>
                </div>
              )}
              {mapping.patterns.length > 0 && (
                <div>
                  <span className="text-[10px] font-semibold text-gray-400 uppercase tracking-wider">Patterns</span>
                  <div className="flex flex-wrap gap-1 mt-0.5">
                    {mapping.patterns.map((pat, idx) => (
                      <span
                        key={idx}
                        className="text-xs px-2 py-0.5 rounded-full bg-purple-100 dark:bg-purple-900/30 text-purple-700 dark:text-purple-300"
                      >
                        {pat}
                      </span>
                    ))}
                  </div>
                </div>
              )}
              {mapping.decisions.length > 0 && (
                <div>
                  <span className="text-[10px] font-semibold text-gray-400 uppercase tracking-wider">ADRs</span>
                  <div className="flex flex-wrap gap-1 mt-0.5">
                    {mapping.decisions.map((dec, idx) => (
                      <span
                        key={idx}
                        className="text-xs px-2 py-0.5 rounded-full bg-green-100 dark:bg-green-900/30 text-green-700 dark:text-green-300"
                      >
                        {dec}
                      </span>
                    ))}
                  </div>
                </div>
              )}
            </div>
          </div>
        ))}
      </div>
    </CollapsibleSection>
  );
}

export default DesignDocPanel;
