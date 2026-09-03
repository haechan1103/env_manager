use super::*;

impl AppRuntime {
    pub fn start_watching(
        &self,
        app: &AppHandle,
        project_id: &str,
        managed_relative_paths: &[String],
    ) -> EnvResult<()> {
        let root = self.root(project_id)?;
        let mut absolute_paths = BTreeSet::new();
        for relative in managed_relative_paths {
            let absolute = root
                .join(relative)
                .canonicalize()
                .map_err(|error| EnvError::io(Path::new(relative), error))?;
            if absolute.starts_with(&root) {
                absolute_paths.insert(absolute);
            }
        }

        let (sender, receiver) = mpsc::channel::<PathBuf>();
        let managed_paths_for_events = absolute_paths.clone();
        let root_for_filter = root.clone();
        let ignored_directories = DiscoveryOptions::default().ignored_directories;
        let mut watcher = RecommendedWatcher::new(
            move |result: Result<notify::Event, notify::Error>| {
                if let Ok(event) = result {
                    for path in event.paths {
                        if should_rescan_for_event(
                            &root_for_filter,
                            &path,
                            &event.kind,
                            &managed_paths_for_events,
                            &ignored_directories,
                        ) {
                            let _ = sender.send(path);
                        }
                    }
                }
            },
            Config::default(),
        )
        .map_err(|_| EnvError::invalid("파일 감시기를 시작하지 못했습니다."))?;

        watcher
            .watch(&root, RecursiveMode::Recursive)
            .map_err(|_| EnvError::invalid("env 파일을 감시하지 못했습니다."))?;

        let app = app.clone();
        let root_for_events = root.clone();
        let project_id_for_events = project_id.to_owned();
        std::thread::spawn(move || {
            while let Ok(first) = receiver.recv() {
                let mut changed = BTreeSet::from([first]);
                while let Ok(next) = receiver.recv_timeout(Duration::from_millis(400)) {
                    changed.insert(next);
                }
                let paths = changed
                    .into_iter()
                    .filter_map(|path| {
                        path.strip_prefix(&root_for_events)
                            .ok()
                            .map(to_relative_string)
                    })
                    .collect::<Vec<_>>();
                if !paths.is_empty() {
                    let _ = app.emit(
                        "managed-files-changed",
                        ManagedFilesChanged {
                            project_id: project_id_for_events.clone(),
                            paths,
                        },
                    );
                }
            }
        });

        self.watchers
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .insert(project_id.to_owned(), watcher);
        Ok(())
    }
}
