import { useFocusEffect } from "expo-router";
import { useCallback, useMemo, useState } from "react";
import {
  FlatList,
  Modal,
  Pressable,
  StyleSheet,
  Text,
  TextInput,
  View,
} from "react-native";
import { SafeAreaView } from "react-native-safe-area-context";
import { useTheme } from "@/hooks/use-theme";
import { AutomationButton } from "./automation-controls";
export function ArtifactFilter({
  label,
  value,
  choices,
  disabled,
  onChange,
}: {
  label: string;
  value: string | null;
  choices: { id: string; name: string }[];
  disabled: boolean;
  onChange: (value: string | null) => void;
}) {
  const theme = useTheme();
  const [open, setOpen] = useState(false),
    [search, setSearch] = useState("");
  useFocusEffect(
    useCallback(
      () => () => {
        setOpen(false);
        setSearch("");
      },
      [],
    ),
  );
  const filtered = useMemo(
    () =>
      choices
        .filter((choice) =>
          choice.name.toLowerCase().includes(search.toLowerCase()),
        )
        .slice(0, 50),
    [choices, search],
  );
  const selected = choices.find((choice) => choice.id === value)?.name;
  const choose = (id: string | null) => {
    setOpen(false);
    setSearch("");
    onChange(id);
  };
  return (
    <>
      <AutomationButton
        label={
          value
            ? `${label}: ${selected ?? "Selected"}`
            : `All ${label.toLowerCase()}`
        }
        disabled={disabled}
        onPress={() => {
          setSearch("");
          setOpen(true);
        }}
      />
      <Modal
        visible={open && !disabled}
        animationType="none"
        onRequestClose={() => setOpen(false)}
      >
        <SafeAreaView style={{ flex: 1, backgroundColor: theme.background }}>
          <View style={styles.header}>
            <Text
              accessibilityRole="header"
              style={[styles.title, { color: theme.text }]}
            >
              {label}
            </Text>
            <AutomationButton label="Cancel" onPress={() => setOpen(false)} />
          </View>
          <TextInput
            autoFocus
            accessibilityLabel={`Search ${label.toLowerCase()}`}
            placeholder={`Search ${label.toLowerCase()}`}
            placeholderTextColor={theme.textSecondary}
            value={search}
            onChangeText={setSearch}
            maxLength={160}
            style={[
              styles.search,
              { color: theme.text, borderColor: theme.border },
            ]}
          />
          <FlatList
            data={filtered}
            keyExtractor={(item) => item.id}
            ListHeaderComponent={
              <AutomationButton
                label={`All ${label.toLowerCase()}`}
                onPress={() => choose(null)}
              />
            }
            ListEmptyComponent={
              <Text style={[styles.empty, { color: theme.textSecondary }]}>
                No matching {label.toLowerCase()}.
              </Text>
            }
            ListFooterComponent={
              filtered.length === 50 ? (
                <Text style={[styles.empty, { color: theme.textSecondary }]}>
                  Search to narrow these results.
                </Text>
              ) : null
            }
            renderItem={({ item }) => (
              <Pressable
                accessibilityRole="button"
                accessibilityLabel={item.name}
                accessibilityState={{ selected: value === item.id }}
                onPress={() => choose(item.id)}
                style={({ pressed }) => [
                  styles.row,
                  {
                    backgroundColor:
                      pressed || value === item.id
                        ? theme.surfaceMuted
                        : "transparent",
                  },
                ]}
              >
                <Text style={{ color: theme.text, fontSize: 16 }}>
                  {item.name}
                </Text>
              </Pressable>
            )}
          />
        </SafeAreaView>
      </Modal>
    </>
  );
}
const styles = StyleSheet.create({
  header: {
    padding: 20,
    flexDirection: "row",
    alignItems: "center",
    justifyContent: "space-between",
  },
  title: { fontSize: 20, fontWeight: "600" },
  search: {
    marginHorizontal: 20,
    marginBottom: 16,
    borderWidth: 1,
    borderRadius: 8,
    padding: 12,
    minHeight: 44,
    fontSize: 16,
  },
  row: { paddingHorizontal: 20, paddingVertical: 16, minHeight: 48 },
  empty: { padding: 20, fontSize: 15 },
});
