import {
  ActivityIndicator,
  FlatList,
  StyleSheet,
  Text,
  TextInput,
  View,
} from "react-native";

import { AppSymbol } from "./app-symbol";
import { Sheet, SheetRow } from "./sheet";
import { NativeTint, Radius } from "@/constants/theme";
import type { useComposerPicker } from "@/hooks/use-composer-picker";
import { useModelSheetListHeight } from "@/hooks/use-model-sheet-list-height";
import { useTheme } from "@/hooks/use-theme";

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
            ? "AI teams @@"
            : kind === "app"
              ? "Apps @@@"
              : "Files, teams & apps @"
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
        style={{ height: listHeight }}
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
                    ? item.app.name
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
            leading={
              item.kind === "file" ? (
                <AppSymbol
                  name={
                    item.file.is_dir
                      ? { ios: "folder", android: "folder", web: "folder" }
                      : {
                          ios: "doc",
                          android: "description",
                          web: "description",
                        }
                  }
                  size={17}
                  tintColor={theme.textTertiary}
                />
              ) : item.kind === "app" || item.kind === "team" ? (
                <AppSymbol
                  name={
                    item.kind === "team"
                      ? { ios: "person.3", android: "groups", web: "groups" }
                      : { ios: "square.grid.2x2", android: "apps", web: "apps" }
                  }
                  size={17}
                  tintColor={theme.textSecondary}
                />
              ) : undefined
            }
            onPress={() => onSelect(item)}
          />
        )}
      />
    </Sheet>
  );
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
});
