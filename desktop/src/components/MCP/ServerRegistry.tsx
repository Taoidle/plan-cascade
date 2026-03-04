import { clsx } from 'clsx';
import { DownloadIcon, PlusIcon } from '@radix-ui/react-icons';
import { useMcpRegistryController } from './useMcpRegistryController';
import { ServerCard } from './ServerCard';
import { AddServerDialog } from './AddServerDialog';
import { ImportDialog } from './ImportDialog';
import { DiscoverTab } from './DiscoverTab';
import { InstallCatalogDialog } from './InstallCatalogDialog';
import { McpDeleteConfirmDialog, McpExportDialog } from './RegistryDialogs';
import { McpToolbar } from './McpToolbar';
import { McpDiagnosticsPanel } from './McpDiagnosticsPanel';
import { McpToolsDrawer } from './McpToolsDrawer';

export function ServerRegistry() {
  const controller = useMcpRegistryController();
  const {
    t,
    servers,
    loading,
    error,
    serverErrors,
    testingIds,
    connectingIds,
    disconnectingIds,
    togglingIds,
    deletingIds,
    connectedServers,
    connectedServerIds,
    addDialogOpen,
    setAddDialogOpen,
    importDialogOpen,
    setImportDialogOpen,
    editingServer,
    setEditingServer,
    showDiagnostics,
    setShowDiagnostics,
    eventLog,
    runtimeInventory,
    runtimeLoading,
    repairingRuntimes,
    setSelectedToolServerId,
    selectedToolServer,
    toolQuery,
    setToolQuery,
    filteredToolDetails,
    toolDetailsLoading,
    activeTab,
    setActiveTab,
    selectedCatalogItem,
    setSelectedCatalogItem,
    installRecommendedNonce,
    exportDialogOpen,
    setExportDialogOpen,
    pendingDeleteServer,
    setPendingDeleteServer,
    installedCatalogItems,
    refreshAll,
    handleCatalogEvent,
    getEventActionLabel,
    fetchServers,
    handleRepairRuntime,
    handleTest,
    handleToggle,
    handleConnect,
    handleDisconnect,
    handleDelete,
    requestDelete,
    handleServerAdded,
    handleServerUpdated,
    handleImportComplete,
    handleCatalogInstalled,
    handleExport,
    handleTestEnabled,
    openExportDialog,
    openServerEditor,
  } = controller;

  return (
    <div className="h-full flex flex-col">
      <McpToolbar
        loading={loading}
        onRefresh={() => void refreshAll(false)}
        onExport={openExportDialog}
        onImport={() => setImportDialogOpen(true)}
        onDiscover={() => setActiveTab('discover')}
        onAdd={() => setAddDialogOpen(true)}
        onTestEnabled={() => void handleTestEnabled()}
      />

      <div className="flex-1 overflow-y-auto p-4">
        <div className="mb-4 inline-flex rounded-lg border border-gray-200 dark:border-gray-700 overflow-hidden">
          <button
            type="button"
            onClick={() => setActiveTab('installed')}
            className={clsx(
              'px-3 py-1.5 text-xs font-medium',
              activeTab === 'installed'
                ? 'bg-primary-600 text-white'
                : 'bg-white dark:bg-gray-900 text-gray-700 dark:text-gray-300',
            )}
          >
            {t('mcp.tabs.installed')}
          </button>
          <button
            type="button"
            onClick={() => setActiveTab('discover')}
            className={clsx(
              'px-3 py-1.5 text-xs font-medium border-l border-gray-200 dark:border-gray-700',
              activeTab === 'discover'
                ? 'bg-primary-600 text-white'
                : 'bg-white dark:bg-gray-900 text-gray-700 dark:text-gray-300',
            )}
          >
            {t('mcp.tabs.discover')}
          </button>
        </div>

        {activeTab === 'discover' ? (
          <DiscoverTab
            onInstallItem={(item) => setSelectedCatalogItem(item)}
            installRecommendedNonce={installRecommendedNonce}
            installedCatalogItems={installedCatalogItems}
            onCatalogEvent={handleCatalogEvent}
          />
        ) : loading && servers.length === 0 ? (
          <div className="space-y-4">
            <ServerSkeleton />
            <ServerSkeleton />
          </div>
        ) : error ? (
          <div className="text-center py-8">
            <p className="text-sm text-red-500 dark:text-red-400">{error}</p>
            <button
              type="button"
              onClick={() => void fetchServers()}
              className="mt-2 text-sm text-primary-600 dark:text-primary-400 hover:underline"
            >
              {t('common.retry')}
            </button>
          </div>
        ) : servers.length === 0 ? (
          <div className="text-center py-12">
            <div className="w-16 h-16 mx-auto mb-4 rounded-full bg-gray-100 dark:bg-gray-800 flex items-center justify-center">
              <PlusIcon className="w-8 h-8 text-gray-400" />
            </div>
            <h3 className="text-lg font-medium text-gray-900 dark:text-white mb-2">{t('mcp.noServers')}</h3>
            <p className="text-sm text-gray-500 dark:text-gray-400 mb-4">{t('mcp.noServersDescription')}</p>
            <div className="flex items-center justify-center gap-2">
              <button
                type="button"
                onClick={() => setImportDialogOpen(true)}
                className={clsx(
                  'flex items-center gap-1.5 px-3 py-2 rounded-md',
                  'bg-gray-100 dark:bg-gray-800 text-gray-700 dark:text-gray-300',
                  'hover:bg-gray-200 dark:hover:bg-gray-700 text-sm font-medium transition-colors',
                )}
              >
                <DownloadIcon className="w-4 h-4" />
                {t('mcp.importFromClaude')}
              </button>
              <button
                type="button"
                onClick={() => setAddDialogOpen(true)}
                className="flex items-center gap-1.5 px-3 py-2 rounded-md bg-primary-600 hover:bg-primary-700 text-white text-sm font-medium transition-colors"
              >
                <PlusIcon className="w-4 h-4" />
                {t('mcp.addManually')}
              </button>
            </div>
          </div>
        ) : (
          <>
            <div className="grid gap-4 grid-cols-1 lg:grid-cols-2 3xl:grid-cols-3">
              {servers.map((server) => (
                <div key={server.id} className="space-y-2">
                  <ServerCard
                    server={server}
                    connected={connectedServerIds.has(server.id)}
                    connectedInfo={connectedServers[server.id]}
                    onTest={() => void handleTest(server.id)}
                    onToggle={(enabled) => void handleToggle(server.id, enabled)}
                    onConnect={() => void handleConnect(server.id)}
                    onDisconnect={() => void handleDisconnect(server.id)}
                    onEdit={() => void openServerEditor(server.id)}
                    onDelete={() => requestDelete(server.id)}
                    onViewTools={
                      connectedServers[server.id]
                        ? () => {
                            setSelectedToolServerId(server.id);
                          }
                        : undefined
                    }
                    isConnecting={connectingIds.has(server.id)}
                    isDisconnecting={disconnectingIds.has(server.id)}
                    isTesting={testingIds.has(server.id)}
                    isToggling={togglingIds.has(server.id)}
                    isDeleting={deletingIds.has(server.id)}
                  />
                  {serverErrors[server.id] && (
                    <div className="text-xs rounded-md border border-red-200 bg-red-50 dark:border-red-900/40 dark:bg-red-900/20 text-red-700 dark:text-red-300 px-2 py-1.5">
                      {serverErrors[server.id]}
                    </div>
                  )}
                </div>
              ))}
            </div>

            <McpDiagnosticsPanel
              showDiagnostics={showDiagnostics}
              onToggle={() => setShowDiagnostics((prev) => !prev)}
              connectedServers={connectedServers}
              runtimeLoading={runtimeLoading}
              runtimeInventory={runtimeInventory}
              repairingRuntimes={repairingRuntimes}
              onRepairRuntime={(runtimeKind) => void handleRepairRuntime(runtimeKind)}
              eventLog={eventLog}
              getEventActionLabel={getEventActionLabel}
            />
          </>
        )}
      </div>

      <McpToolsDrawer
        open={!!selectedToolServer}
        server={selectedToolServer}
        tools={filteredToolDetails}
        query={toolQuery}
        onQueryChange={setToolQuery}
        loading={toolDetailsLoading}
        onClose={() => setSelectedToolServerId(null)}
      />

      <McpExportDialog
        open={exportDialogOpen}
        onOpenChange={setExportDialogOpen}
        onConfirm={() => {
          setExportDialogOpen(false);
          void handleExport();
        }}
      />

      <McpDeleteConfirmDialog
        server={pendingDeleteServer}
        onOpenChange={(open) => {
          if (!open) {
            setPendingDeleteServer(null);
          }
        }}
        onConfirm={() => {
          if (!pendingDeleteServer) return;
          const deletingId = pendingDeleteServer.id;
          setPendingDeleteServer(null);
          void handleDelete(deletingId);
        }}
      />

      <AddServerDialog
        open={addDialogOpen}
        onOpenChange={setAddDialogOpen}
        onServerAdded={handleServerAdded}
        onServerUpdated={handleServerUpdated}
      />

      <AddServerDialog
        open={!!editingServer}
        onOpenChange={(open) => {
          if (!open) {
            setEditingServer(null);
          }
        }}
        onServerAdded={handleServerAdded}
        onServerUpdated={handleServerUpdated}
        server={editingServer}
      />

      <ImportDialog
        open={importDialogOpen}
        onOpenChange={setImportDialogOpen}
        onImportComplete={handleImportComplete}
      />

      <InstallCatalogDialog
        open={!!selectedCatalogItem}
        item={selectedCatalogItem}
        onOpenChange={(open) => {
          if (!open) {
            setSelectedCatalogItem(null);
          }
        }}
        onInstalled={handleCatalogInstalled}
      />
    </div>
  );
}

function ServerSkeleton() {
  return (
    <div className="p-4 rounded-lg border border-gray-200 dark:border-gray-700" aria-hidden="true">
      <div className="flex items-start justify-between mb-3">
        <div className="flex items-center gap-3">
          <div className="w-2.5 h-2.5 rounded-full bg-gray-200 dark:bg-gray-700 animate-skeleton" />
          <div>
            <div className="h-5 w-32 bg-gray-200 dark:bg-gray-700 rounded mb-1 animate-skeleton" />
            <div className="h-4 w-20 bg-gray-100 dark:bg-gray-800 rounded animate-skeleton" />
          </div>
        </div>
        <div className="h-5 w-9 bg-gray-200 dark:bg-gray-700 rounded-full animate-skeleton" />
      </div>
      <div className="h-4 w-48 bg-gray-100 dark:bg-gray-800 rounded mb-3 animate-skeleton" />
      <div className="flex gap-2">
        <div className="h-7 w-16 bg-gray-100 dark:bg-gray-800 rounded animate-skeleton" />
        <div className="h-7 w-16 bg-gray-100 dark:bg-gray-800 rounded animate-skeleton" />
      </div>
    </div>
  );
}
