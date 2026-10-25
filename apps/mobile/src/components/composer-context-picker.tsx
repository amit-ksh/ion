import type { ComposerAutocompleteRow } from "@waku/client/composer-autocomplete";
import type { ReactNode } from "react";
import {
  ActivityIndicator,
  FlatList,
  StyleSheet,
  Text,
  TextInput,
  View,
  useWindowDimensions,
} from "react-native";

import { AppIcon } from "./app-icon";
import { AppSymbol } from "./app-symbol";
import { Sheet, SheetRow } from "./sheet";
import { MonoFont, NativeTint, Radius } from "@/constants/theme";
import type { useComposerPicker } from "@/hooks/use-composer-picker";
import { useModelSheetListHeight } from "@/hooks/use-model-sheet-list-height";
import { useTheme } from "@/hooks/use-theme";
import { appDisplayName } from "@/lib/app-display-name";

type SymbolName = Parameters<typeof AppSymbol>[0]["name"];

const LEGEND = [["@", "anything"], ["@@", "teams"], ["@@@", "apps"]] as const;
const LEGEND_HEIGHT = 44;

export function ComposerContextPicker({
  kind,
  visible,
  query,
  onQueryChange,
  rows,
  hasResults,
  loading,
  error,
  warning,
  onSelect,
  onDismiss,
  onRetry,
}: ReturnType<typeof useComposerPicker>["picker"]) {
  const theme = useTheme();
  const { fontScale } = useWindowDimensions();
  const listHeight = useModelSheetListHeight(visible);
  const commands = kind === "command";
  return (
    <Sheet
      visible={visible}
      onDismiss={onDismiss}
      scrollable={false}
      title={
        commands
          ? "Commands"
          : kind === "team"
            ? "AI teams"
            : kind === "app"
              ? "Apps"
              : "Files, teams and apps"
      }
    >
      <View style={[styles.search, { backgroundColor: theme.inset }]}>
        <TextInput
          accessibilityLabel={
            commands
              ? "Search commands"
              : kind === "team"
                ? "Search AI teams"
                : kind === "app"
                  ? "Search connected apps"
                  : "Search files, teams and apps"
          }
          autoCapitalize="none"
          autoCorrect={false}
          placeholder={
            commands
              ? "Search commands…"
              : kind === "team"
                ? "Search AI teams…"
                : kind === "app"
                  ? "Search connected apps…"
                  : "Search files, teams and apps…"
          }
          placeholderTextColor={theme.textTertiary}
          selectionColor={NativeTint}
          style={[styles.searchInput, { color: theme.text }]}
          value={query}
          onChangeText={onQueryChange}
        />
        <View style={styles.searchProgress}>
          {loading && (
            <ActivityIndicator
              accessibilityLabel="Searching"
              color={theme.textTertiary}
              size="small"
            />
          )}
        </View>
      </View>
      <FlatList
        data={rows}
        initialNumToRender={8}
        keyboardDismissMode="on-drag"
        keyboardShouldPersistTaps="handled"
        // The legend keeps its place under the list, so the list gives up its height.
        style={{ height: Math.max(0, listHeight - (commands ? 0 : Math.ceil(LEGEND_HEIGHT * Math.max(1, fontScale)))) }}
        keyExtractor={(row) =>
          row.kind === "command"
            ? `command:${row.command.name}`
            : row.kind === "team"
              ? `team:${row.team.id}`
              : row.kind === "app"
                ? `app:${row.app.slug}`
                : `file:${row.file.path}`
        }
        ListHeaderComponent={
          warning ? (
            <>
              <Text
                accessibilityLiveRegion="polite"
                style={[styles.note, { color: theme.textSecondary }]}
              >
                {warning}
              </Text>
              <SheetRow label="Retry apps" onPress={onRetry} />
            </>
          ) : null
        }
        ListEmptyComponent={
          error ? (
            <>
              <Text
                accessibilityLiveRegion="polite"
                style={[styles.note, { color: theme.danger }]}
              >
                {error}
              </Text>
              <SheetRow label="Retry" onPress={onRetry} />
            </>
          ) : (
            <Text style={[styles.note, { color: theme.textTertiary }]}>
              {loading && !hasResults
                ? "Loading suggestions…"
                : commands
                  ? "No matching commands"
                  : kind === "team"
                    ? "No matching AI teams. Create a team in Teams."
                    : kind === "app"
                      ? "No matching apps. Connect an app in Apps."
                      : "No matching files, teams or apps"}
            </Text>
          )
        }
        renderItem={({ item }) => (
          <SheetRow
            label={
              item.kind === "command"
                ? `/${item.command.name}`
                : item.kind === "team"
                  ? item.team.name
                  : item.kind === "app"
                    ? appDisplayName(item.app.slug, item.app.name)
                    : item.file.path
            }
            description={
              item.kind === "command"
                ? [
                    item.command.scope.toLowerCase(),
                    item.command.argument_hint,
                    item.command.description,
                  ]
                    .filter(Boolean)
                    .join(" · ")
                : item.kind === "team"
                  ? item.invoke
                    ? "Team · starts a new team task"
                    : "Team reference"
                  : item.kind === "app"
                    ? "Connected app"
                    : item.file.is_dir
                      ? "Folder"
                      : "File"
            }
            leading={<RowIcon row={item} />}
            onPress={() => onSelect(item)}
          />
        )}
      />
      {!commands && (
        <View
          accessible
          accessibilityLabel="In a message, type @ to mention anything, @@ for teams, or @@@ for apps"
          style={[styles.legend, { borderTopColor: theme.border, minHeight: LEGEND_HEIGHT }]}
        >
          <Text style={[styles.legendText, { color: theme.textTertiary }]}>Type</Text>
          {LEGEND.map(([sigil, label]) => (
            <View key={sigil} style={styles.legendItem}>
              <Text style={[styles.sigil, { backgroundColor: theme.overlayStrong, color: theme.text }]}>
                {sigil}
              </Text>
              <Text style={[styles.legendText, { color: theme.textSecondary }]}>{label}</Text>
            </View>
          ))}
        </View>
      )}
    </Sheet>
  );
}

/** Fixed 32dp tile so every row's label starts on the same column. */
function RowIcon({ row }: { row: ComposerAutocompleteRow }) {
  const theme = useTheme();
  let icon: ReactNode;
  if (row.kind === "app") icon = <AppIcon toolkit={row.app.slug} size={18} />;
  else {
    const name: SymbolName = row.kind === "file"
      ? fileSymbol(row.file.path, row.file.is_dir)
      : row.kind === "team"
        ? { ios: "person.3", android: "groups", web: "groups" }
        : { ios: "terminal", android: "terminal", web: "terminal" };
    icon = <AppSymbol name={name} size={18} tintColor={theme.textSecondary} />;
  }
  return (
    <View
      accessibilityElementsHidden
      importantForAccessibility="no-hide-descendants"
      style={[styles.tile, { backgroundColor: theme.overlayStrong }]}
    >
      {icon}
    </View>
  );
}

function fileSymbol(path: string, isDir: boolean): SymbolName {
  if (isDir) return { ios: "folder", android: "folder", web: "folder" };
  const name = path.slice(path.lastIndexOf("/") + 1).toLowerCase();
  const ext = name.includes(".") ? name.slice(name.lastIndexOf(".") + 1) : "";
  if (/^(png|jpe?g|gif|webp|svg|heic|ico|bmp)$/.test(ext))
    return { ios: "photo", android: "image", web: "image" };
  if (/^(md|mdx|txt|rst)$/.test(ext))
    return { ios: "doc.text", android: "article", web: "article" };
  if (/^(json|jsonc|ya?ml|toml|xml|lock|ini|env)$/.test(ext))
    return { ios: "curlybraces", android: "data_object", web: "data_object" };
  if (/^(csv|tsv|xlsx?)$/.test(ext))
    return { ios: "tablecells", android: "table_chart", web: "table_chart" };
  if (ext === "pdf")
    return { ios: "doc.richtext", android: "picture_as_pdf", web: "picture_as_pdf" };
  if (/^(ts|tsx|js|jsx|mjs|cjs|rs|py|go|java|kt|swift|c|cc|cpp|h|hpp|cs|rb|php|sh|ps1|sql|css|scss|html|vue|svelte)$/.test(ext))
    return { ios: "chevron.left.forwardslash.chevron.right", android: "code", web: "code" };
  return { ios: "doc", android: "description", web: "description" };
}

const styles = StyleSheet.create({
  search: {
    borderRadius: Radius.medium,
    flexDirection: "row",
    alignItems: "center",
    paddingLeft: 12,
    paddingRight: 8,
    marginHorizontal: 6,
    marginBottom: 8,
  },
  searchInput: { flex: 1, fontSize: 15, minHeight: 44, paddingVertical: 10 },
  searchProgress: { width: 28, alignItems: "center", justifyContent: "center" },
  note: {
    fontSize: 13,
    lineHeight: 18,
    paddingHorizontal: 12,
    paddingVertical: 14,
  },
  tile: {
    alignItems: "center",
    borderRadius: Radius.small,
    height: 32,
    justifyContent: "center",
    width: 32,
  },
  legend: {
    alignItems: "center",
    borderTopWidth: StyleSheet.hairlineWidth,
    columnGap: 12,
    flexDirection: "row",
    flexWrap: "wrap",
    marginHorizontal: 6,
    paddingHorizontal: 6,
    paddingTop: 8,
    rowGap: 4,
  },
  legendItem: { alignItems: "center", flexDirection: "row", gap: 5 },
  legendText: { fontSize: 12.5, lineHeight: 18 },
  sigil: {
    borderRadius: 5,
    fontFamily: MonoFont,
    fontSize: 12,
    lineHeight: 18,
    overflow: "hidden",
    paddingHorizontal: 5,
  },
});
