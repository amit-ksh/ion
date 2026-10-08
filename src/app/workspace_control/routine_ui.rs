//! Calendar editing stays in memory. The owner computes zoned previews on a worker.
use super::*;

#[derive(Clone, Copy, Default, Eq, PartialEq)]
enum Frequency {
    #[default]
    Interval,
    Daily,
    Weekly,
    Monthly,
}
impl Frequency {
    fn label(self) -> &'static str {
        match self {
            Self::Interval => "Every few minutes",
            Self::Daily => "Daily",
            Self::Weekly => "Days of the week",
            Self::Monthly => "Days of the month",
        }
    }
}
const WEEKDAYS: [&str; 7] = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];
pub(super) struct RoutineUi {
    frequency: Frequency,
    zone: Entity<TextInput>,
    time: Entity<TextInput>,
    month_days: Entity<TextInput>,
    weekdays: Vec<u8>,
    missed: MissedRunPolicy,
    overlap: OverlapPolicy,
    preview: Option<RoutineSchedulePreview>,
    busy: bool,
    generation: u64,
    error: Option<String>,
    subscribed: bool,
}
impl RoutineUi {
    pub(super) fn new(window: &mut Window, cx: &mut App) -> Self {
        Self {
            frequency: Frequency::Interval,
            zone: cx.new(|cx| TextInput::new(window, cx).placeholder("Asia/Kolkata")),
            time: cx.new(|cx| TextInput::new(window, cx).placeholder("09:00")),
            month_days: cx.new(|cx| TextInput::new(window, cx).placeholder("1, 15")),
            weekdays: vec![0, 1, 2, 3, 4],
            missed: MissedRunPolicy::default(),
            overlap: OverlapPolicy::default(),
            preview: None,
            busy: false,
            generation: 0,
            error: None,
            subscribed: false,
        }
    }
    fn invalidate(&mut self) {
        self.preview = None;
        self.error = None;
    }
}

/// This label formats the saved rule only; it never resolves a time zone on a frame.
pub(super) fn schedule_label(interval: u32, calendar: Option<&CalendarSchedule>) -> String {
    let Some(rule) = calendar else {
        return format!("Every {interval} minutes");
    };
    let recurrence = match &rule.recurrence {
        CalendarRecurrence::Daily => "Daily".into(),
        CalendarRecurrence::Weekly { days } => days
            .iter()
            .filter_map(|d| WEEKDAYS.get(usize::from(*d)))
            .copied()
            .collect::<Vec<_>>()
            .join(", "),
        CalendarRecurrence::Monthly { days } => format!(
            "Monthly on {}",
            days.iter()
                .map(u8::to_string)
                .collect::<Vec<_>>()
                .join(", ")
        ),
    };
    format!(
        "{recurrence} at {:02}:{:02} · {}",
        rule.hour, rule.minute, rule.time_zone
    )
}
pub(super) fn trigger_label(trigger: &RoutineTrigger) -> String {
    format!(
        "{} · {} · {}",
        schedule_label(trigger.interval_minutes, trigger.calendar.as_ref()),
        match trigger.missed_run_policy {
            MissedRunPolicy::RunLatest => "Run latest missed time",
            MissedRunPolicy::Skip => "Skip missed times",
        },
        match trigger.overlap_policy {
            OverlapPolicy::Skip => "Skip overlap",
            OverlapPolicy::Allow => "Allow overlap",
        }
    )
}

fn parse_schedule(
    frequency: Frequency,
    interval: &str,
    time: &str,
    zone: &str,
    month_days: &str,
    weekdays: &[u8],
) -> Result<RoutineScheduleRequest, String> {
    if interval.len() > 6 {
        return Err("Enter an interval from 1 to 525600 minutes.".into());
    }
    let interval_minutes = interval
        .trim()
        .parse::<u32>()
        .ok()
        .filter(|n| (1..=525600).contains(n))
        .ok_or("Enter an interval from 1 to 525600 minutes.")?;
    if frequency == Frequency::Interval {
        return Ok(RoutineScheduleRequest {
            interval_minutes,
            calendar: None,
        });
    }
    if time.len() != 5
        || time.as_bytes().get(2) != Some(&b':')
        || !time
            .as_bytes()
            .iter()
            .enumerate()
            .all(|(i, b)| i == 2 || b.is_ascii_digit())
    {
        return Err("Enter a time in 24-hour HH:MM format.".into());
    }
    let hour = time[..2].parse::<u8>().unwrap();
    let minute = time[3..].parse::<u8>().unwrap();
    if hour > 23 || minute > 59 {
        return Err("Enter a valid 24-hour time.".into());
    }
    if zone.len() > 128 || zone.trim().is_empty() {
        return Err("Enter an IANA time zone, such as Asia/Kolkata.".into());
    }
    let recurrence = match frequency {
        Frequency::Daily => CalendarRecurrence::Daily,
        Frequency::Weekly => {
            if weekdays.is_empty() || weekdays.len() > 7 || weekdays.iter().any(|d| *d > 6) {
                return Err("Choose at least one weekday.".into());
            }
            CalendarRecurrence::Weekly {
                days: weekdays.to_vec(),
            }
        }
        Frequency::Monthly => {
            if month_days.len() > 96 {
                return Err("Enter up to 31 unique days, separated by commas.".into());
            }
            let mut days = Vec::new();
            for day in month_days.split(',').take(32) {
                let day = day
                    .trim()
                    .parse::<u8>()
                    .ok()
                    .filter(|d| (1..=31).contains(d))
                    .ok_or("Enter days from 1 to 31, separated by commas.")?;
                if days.contains(&day) || days.len() == 31 {
                    return Err("Enter each day once.".into());
                }
                days.push(day);
            }
            days.sort_unstable();
            CalendarRecurrence::Monthly { days }
        }
        Frequency::Interval => unreachable!(),
    };
    Ok(RoutineScheduleRequest {
        interval_minutes,
        calendar: Some(CalendarSchedule {
            time_zone: zone.trim().to_owned(),
            hour,
            minute,
            recurrence,
        }),
    })
}
impl Waku {
    pub(super) fn reset_routine_editor(&mut self, cx: &mut Context<Self>) {
        if !self.control_ui.routine.subscribed {
            for input in [
                &self.control_ui.interval,
                &self.control_ui.routine.zone,
                &self.control_ui.routine.time,
                &self.control_ui.routine.month_days,
            ] {
                cx.subscribe(input, |this: &mut Self, _, event: &InputEvent, cx| {
                    if matches!(event, InputEvent::Edited) {
                        this.control_ui.routine.invalidate();
                        cx.notify();
                    }
                })
                .detach();
            }
            self.control_ui.routine.subscribed = true;
        }
        let r = &mut self.control_ui.routine;
        r.generation += 1;
        r.busy = false;
        r.invalidate();
        r.frequency = Frequency::Interval;
        r.weekdays = vec![0, 1, 2, 3, 4];
        r.missed = MissedRunPolicy::default();
        r.overlap = OverlapPolicy::default();
        r.zone.update(cx, |i, cx| i.set_content("UTC", cx));
        r.time.update(cx, |i, cx| i.set_content("09:00", cx));
        r.month_days.update(cx, |i, cx| i.set_content("1", cx));
        self.control_ui
            .interval
            .update(cx, |i, cx| i.set_content("60", cx));
    }
    pub(super) fn load_routine_editor(&mut self, routine: &Routine, cx: &mut Context<Self>) {
        let r = &mut self.control_ui.routine;
        r.missed = routine.missed_run_policy;
        r.overlap = routine.overlap_policy;
        if let Some(rule) = &routine.calendar {
            r.zone
                .update(cx, |i, cx| i.set_content(rule.time_zone.clone(), cx));
            r.time.update(cx, |i, cx| {
                i.set_content(format!("{:02}:{:02}", rule.hour, rule.minute), cx)
            });
            r.frequency = match &rule.recurrence {
                CalendarRecurrence::Daily => Frequency::Daily,
                CalendarRecurrence::Weekly { days } => {
                    r.weekdays = days.clone();
                    Frequency::Weekly
                }
                CalendarRecurrence::Monthly { days } => {
                    r.month_days.update(cx, |i, cx| {
                        i.set_content(
                            days.iter()
                                .map(u8::to_string)
                                .collect::<Vec<_>>()
                                .join(", "),
                            cx,
                        )
                    });
                    Frequency::Monthly
                }
            };
        }
    }
    pub(super) fn routine_schedule_draft(
        &self,
        cx: &App,
    ) -> Result<RoutineScheduleRequest, String> {
        let r = &self.control_ui.routine;
        parse_schedule(
            r.frequency,
            self.control_ui.interval.read(cx).content(),
            r.time.read(cx).content(),
            r.zone.read(cx).content(),
            r.month_days.read(cx).content(),
            &r.weekdays,
        )
    }
    pub(super) fn routine_policies(&self) -> (MissedRunPolicy, OverlapPolicy) {
        (
            self.control_ui.routine.missed,
            self.control_ui.routine.overlap,
        )
    }
    fn preview_routine_schedule(&mut self, cx: &mut Context<Self>) {
        if self.control_ui.routine.busy {
            return;
        }
        let request = match self.routine_schedule_draft(cx) {
            Ok(request) => request,
            Err(error) => {
                self.control_ui.routine.error = Some(error);
                cx.notify();
                return;
            }
        };
        let r = &mut self.control_ui.routine;
        r.invalidate();
        r.busy = true;
        r.generation += 1;
        let generation = r.generation;
        let editor = self.control_ui.editor_generation;
        let command = Rpc::PreviewRoutineSchedule {
            request: request.clone(),
        };
        let client = self.daemon.client();
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_executor()
                .spawn(async move {
                    smol::unblock(move || -> anyhow::Result<_> {
                        let ResponsePayload::RoutineSchedulePreview { preview } =
                            client.request(Uuid::nil(), Uuid::nil(), command)?
                        else {
                            anyhow::bail!("Unexpected schedule response")
                        };
                        Ok(preview)
                    })
                    .await
                })
                .await;
            let _ = this.update(cx, |this, cx| {
                if this.control_ui.editor_generation != editor
                    || this.control_ui.routine.generation != generation
                {
                    return;
                }
                this.control_ui.routine.busy = false;
                if this.routine_schedule_draft(cx).as_ref() == Ok(&request) {
                    match result {
                        Ok(preview) => this.control_ui.routine.preview = Some(preview),
                        Err(error) => this.control_ui.routine.error = Some(error.to_string()),
                    }
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }
    pub(super) fn render_routine_schedule(&self, cx: &mut Context<Self>) -> Div {
        let theme = Theme::current(cx);
        let r = &self.control_ui.routine;
        let weak = cx.entity().downgrade();
        let selected = r.frequency;
        let menu = self.menu_handle("routine-frequency", cx);
        let mut form = div().flex().flex_col().gap(px(12.0)).child(dropdown_menu(
            self.control_button("routine-frequency", selected.label(), cx, |_, _, _| {}),
            "routine-frequency",
            &menu,
            MenuAlign::BelowLeft,
            move |_| {
                [
                    Frequency::Interval,
                    Frequency::Daily,
                    Frequency::Weekly,
                    Frequency::Monthly,
                ]
                .into_iter()
                .map(|frequency| {
                    let weak = weak.clone();
                    MenuItem::new(frequency.label(), move |_, cx| {
                        let _ = weak.update(cx, |this, cx| {
                            this.control_ui.routine.frequency = frequency;
                            this.control_ui.routine.invalidate();
                            cx.notify();
                        });
                    })
                    .selected(frequency == selected)
                })
                .collect()
            },
        ));
        if r.frequency == Frequency::Interval {
            form = form.child(control_field(
                "Every (minutes)",
                self.control_ui.interval.clone(),
                "control-interval",
            ));
        } else {
            form = form
                .child(control_field(
                    "Time (24-hour)",
                    r.time.clone(),
                    "routine-time",
                ))
                .child(control_field("Time zone", r.zone.clone(), "routine-zone"));
            if r.frequency == Frequency::Weekly {
                let mut days = div().flex().flex_wrap().gap(px(6.0));
                for (day, label) in WEEKDAYS.into_iter().enumerate() {
                    days = days.child(self.control_button(
                        format!("routine-weekday-{day}"),
                        format!(
                            "{} {label}",
                            if r.weekdays.contains(&(day as u8)) {
                                "✓"
                            } else {
                                "+"
                            }
                        ),
                        cx,
                        move |this, _, cx| {
                            let r = &mut this.control_ui.routine;
                            if r.weekdays.contains(&(day as u8)) {
                                r.weekdays.retain(|d| *d != day as u8);
                            } else {
                                r.weekdays.push(day as u8);
                                r.weekdays.sort_unstable();
                            }
                            r.invalidate();
                            cx.notify();
                        },
                    ));
                }
                form = form.child(days);
            } else if r.frequency == Frequency::Monthly {
                form = form.child(control_field(
                    "Days of the month (1–31)",
                    r.month_days.clone(),
                    "routine-month-days",
                ));
            }
        }
        for missed_menu in [true, false] {
            let id = if missed_menu {
                "routine-missed"
            } else {
                "routine-overlap"
            };
            let label = if missed_menu {
                match r.missed {
                    MissedRunPolicy::RunLatest => "Missed run: run latest",
                    MissedRunPolicy::Skip => "Missed run: skip",
                }
            } else {
                match r.overlap {
                    OverlapPolicy::Skip => "Overlap: skip",
                    OverlapPolicy::Allow => "Overlap: allow",
                }
            };
            let selected_missed = r.missed;
            let selected_overlap = r.overlap;
            let weak = cx.entity().downgrade();
            let menu = self.menu_handle(id, cx);
            form = form.child(dropdown_menu(
                self.control_button(id, label, cx, |_, _, _| {}),
                id,
                &menu,
                MenuAlign::BelowLeft,
                move |_| {
                    if missed_menu {
                        [MissedRunPolicy::RunLatest, MissedRunPolicy::Skip]
                            .into_iter()
                            .map(|policy| {
                                let weak = weak.clone();
                                MenuItem::new(
                                    match policy {
                                        MissedRunPolicy::RunLatest => "Run the latest missed time",
                                        MissedRunPolicy::Skip => {
                                            "Skip times missed by over a minute"
                                        }
                                    },
                                    move |_, cx| {
                                        let _ = weak.update(cx, |this, cx| {
                                            this.control_ui.routine.missed = policy;
                                            cx.notify();
                                        });
                                    },
                                )
                                .selected(policy == selected_missed)
                            })
                            .collect()
                    } else {
                        [OverlapPolicy::Skip, OverlapPolicy::Allow]
                            .into_iter()
                            .map(|policy| {
                                let weak = weak.clone();
                                MenuItem::new(
                                    match policy {
                                        OverlapPolicy::Skip => {
                                            "Skip while a previous run is active"
                                        }
                                        OverlapPolicy::Allow => {
                                            "Allow overlapping runs within team limits"
                                        }
                                    },
                                    move |_, cx| {
                                        let _ = weak.update(cx, |this, cx| {
                                            this.control_ui.routine.overlap = policy;
                                            cx.notify();
                                        });
                                    },
                                )
                                .selected(policy == selected_overlap)
                            })
                            .collect()
                    }
                },
            ));
        }
        form = form.child(self.control_button(
            "routine-preview",
            if r.busy {
                "Calculating…"
            } else {
                "Preview next runs"
            },
            cx,
            |this, _, cx| this.preview_routine_schedule(cx),
        ));
        if let Some(error) = &r.error {
            form = form.child(
                div()
                    .text_size(sp(12.5))
                    .text_color(theme.text)
                    .child(error.clone()),
            );
        }
        if let Some(preview) = &r.preview {
            for fire in &preview.fires {
                form = form.child(
                    div()
                        .text_size(sp(13.0))
                        .text_color(theme.text)
                        .child(fire.display.clone()),
                );
            }
        }
        form.child(div().text_size(sp(12.5)).text_color(theme.text_secondary).child("Saved routines stay paused until enabled. Calendar times follow the selected zone; unavailable dates and daylight-saving gaps are skipped. Repeated times fire once."))
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn routine_editor_validates_bounded_fields_and_keeps_exact_calendar_rule() {
        let request = parse_schedule(
            Frequency::Monthly,
            "60",
            "09:15",
            "Asia/Kolkata",
            "31, 1",
            &[],
        )
        .unwrap();
        assert_eq!(
            request.calendar.unwrap(),
            CalendarSchedule {
                time_zone: "Asia/Kolkata".into(),
                hour: 9,
                minute: 15,
                recurrence: CalendarRecurrence::Monthly { days: vec![1, 31] }
            }
        );
        for (time, zone, days) in [
            ("09:61", "UTC", "1"),
            ("9:00", "UTC", "1"),
            ("09:00", "UTC", "1,1"),
            ("09:00", "UTC", "0"),
            ("09:00", "", "1"),
            ("éé:0", "UTC", "1"),
        ] {
            assert!(parse_schedule(Frequency::Monthly, "60", time, zone, days, &[]).is_err());
        }
        assert!(parse_schedule(Frequency::Weekly, "60", "09:00", "UTC", "1", &[]).is_err());
        assert!(parse_schedule(Frequency::Interval, "0", "", "", "", &[]).is_err());
        assert!(parse_schedule(Frequency::Interval, "525601", "", "", "", &[]).is_err());
        assert!(
            parse_schedule(Frequency::Interval, "60", "bad", "", "", &[])
                .unwrap()
                .calendar
                .is_none()
        );
    }
}
