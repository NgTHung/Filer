// Compatibility pin for API-006: these harness handlers keep the legacy
// command routes available to the isolated compatibility tests until removal.

    fn register_test_handlers(
        handlers: &Arc<HandlerRegistry>,
        nav_tx: Sender<NavCommand>,
        scan_tx: Sender<ScanCommand>,
        search_tx: Sender<SearchCommand>,
        watch_tx: Sender<WatchCommand>,
        preview_tx: Sender<PreviewCommand>,
        ops_tx: Sender<OpsCommand>,
    ) {
            handlers.on("session.handshake", |_cmd, ctx| {
                let session = ctx.sessions.create_session(ctx.events.clone());
                let _ = ctx.events.send(Event::SessionCreated(session));
            });
            handlers.on("session.destroy", |cmd, ctx| {
                if let Command::DestroySession(session_id) = cmd {
                    ctx.sessions.remove(session_id);
                    let _ = ctx.events.send(Event::SessionDestroyed(session_id));
                }
            });

            {
                let tx = nav_tx.clone();
                handlers.on("navigate.path.compat", move |cmd, _ctx| {
                    if let Command::NavigatePathCompat {
                        path,
                        session,
                        request,
                    } = cmd
                    {
                        let _ = tx.send(NavCommand::NavigateToPath {
                            session,
                            path,
                            request,
                        });
                    }
                });
            }
            {
                let tx = nav_tx.clone();
                handlers.on("navigate", move |cmd, _ctx| {
                    if let Command::Navigate {
                        location,
                        session,
                        request,
                    } = cmd
                    {
                        let _ = tx.send(NavCommand::NavigateToLocation {
                            session,
                            location,
                            request,
                        });
                    }
                });
            }
            {
                let tx = nav_tx.clone();
                handlers.on("navigate.node.compat", move |cmd, _ctx| {
                    if let Command::NavigateNodeCompat {
                        node,
                        session,
                        request,
                    } = cmd
                    {
                        let _ = tx.send(NavCommand::Navigate {
                            session,
                            node,
                            request,
                        });
                    }
                });
            }
            {
                let tx = nav_tx.clone();
                handlers.on("navigate.up", move |cmd, _ctx| {
                    if let Command::NavigateUp { session, request } = cmd {
                        let _ = tx.send(NavCommand::Up(session, request));
                    }
                });
            }
            {
                let tx = nav_tx.clone();
                handlers.on("navigate.back", move |cmd, _ctx| {
                    if let Command::NavigateBack { session, request } = cmd {
                        let _ = tx.send(NavCommand::Back(session, request));
                    }
                });
            }
            {
                let tx = nav_tx.clone();
                handlers.on("navigate.refresh", move |cmd, _ctx| {
                    if let Command::Refresh { session, request } = cmd {
                        let _ = tx.send(NavCommand::Refresh(session, request));
                    }
                });
            }

            {
                let tx = search_tx.clone();
                handlers.on("search.node.compat", move |cmd, ctx| {
                    if let Command::SearchNodeCompat {
                        query,
                        root: node_root,
                        session,
                        request,
                    } = cmd
                    {
                        match crate::model::query::SearchQuery::parse(&query) {
                            Ok(query) => {
                                let Ok(root) =
                                    compat::resolve_node_location(&ctx.registry, node_root)
                                else {
                                    compat::emit_unresolved_node_request(
                                        &ctx.events,
                                        node_root,
                                        session,
                                        request,
                                        "search.node.compat resolve",
                                    );
                                    return;
                                };
                                send_or_warn(
                                    &tx,
                                    SearchCommand::Search {
                                        query,
                                        root,
                                        event_mode: SearchEventMode::Compat,
                                        session,
                                        request,
                                    },
                                    "search.node.compat",
                                );
                            }
                            Err(error) => {
                                send_or_warn(
                                    &ctx.events,
                                    Event::from_request_error(
                                        CoreError::invalid_input(format!(
                                            "Invalid search query: {error}"
                                        )),
                                        session,
                                        request,
                                    ),
                                    "search query parse",
                                );
                            }
                        }
                    }
                });
            }
            {
                let tx = search_tx.clone();
                handlers.on("search.path.compat", move |cmd, ctx| {
                    if let Command::SearchPathCompat {
                        query,
                        root,
                        session,
                        request,
                    } = cmd
                    {
                        match crate::model::query::SearchQuery::parse(&query) {
                            Ok(query) => {
                                let location = crate::Location::local(root);
                                send_or_warn(
                                    &tx,
                                    SearchCommand::Search {
                                        query,
                                        root: crate::LocationRef::from_location(&location),
                                        event_mode: SearchEventMode::Compat,
                                        session,
                                        request,
                                    },
                                    "search.path.compat",
                                );
                            }
                            Err(error) => {
                                send_or_warn(
                                    &ctx.events,
                                    Event::from_request_error(
                                        CoreError::invalid_input(format!(
                                            "Invalid search query: {error}"
                                        )),
                                        session,
                                        request,
                                    ),
                                    "search query parse",
                                );
                            }
                        }
                    }
                });
            }
            {
                let tx = search_tx.clone();
                handlers.on("search", move |cmd, ctx| {
                    if let Command::Search {
                        query,
                        root,
                        session,
                        request,
                    } = cmd
                    {
                        match crate::model::query::SearchQuery::parse(&query) {
                            Ok(query) => {
                                send_or_warn(
                                    &tx,
                                    SearchCommand::Search {
                                        query,
                                        root,
                                        event_mode: SearchEventMode::Location,
                                        session,
                                        request,
                                    },
                                    "search",
                                );
                            }
                            Err(error) => {
                                send_or_warn(
                                    &ctx.events,
                                    Event::from_request_error(
                                        CoreError::invalid_input(format!(
                                            "Invalid search query: {error}"
                                        )),
                                        session,
                                        request,
                                    ),
                                    "search query parse",
                                );
                            }
                        }
                    }
                });
            }
            {
                let tx = search_tx.clone();
                handlers.on("search.cancel", move |cmd, _ctx| {
                    if let Command::CancelSearch { session } = cmd {
                        let _ = tx.send(SearchCommand::Cancel(session));
                    }
                });
            }

            {
                let tx = scan_tx.clone();
                handlers.on("scan.path.compat", move |cmd, _ctx| {
                    if let Command::ScanPathCompat {
                        path,
                        session,
                        pipeline,
                        load,
                        request,
                    } = cmd
                    {
                        let location = crate::Location::local(path);
                        let _ = tx.send(ScanCommand::ScanCompat {
                            location: crate::LocationRef::from_location(&location),
                            session,
                            pipeline,
                            load,
                            request,
                        });
                    }
                });
            }
            {
                let tx = scan_tx.clone();
                handlers.on("scan", move |cmd, _ctx| {
                    if let Command::Scan {
                        location,
                        session,
                        pipeline,
                        load,
                        request,
                    } = cmd
                    {
                        let _ = tx.send(ScanCommand::ScanLocation {
                            location,
                            session,
                            pipeline,
                            load,
                            request,
                        });
                    }
                });
            }
            {
                let tx = scan_tx.clone();
                handlers.on("scan.node.compat", move |cmd, ctx| {
                    if let Command::ScanNodeCompat {
                        node,
                        session,
                        pipeline,
                        load,
                        request,
                    } = cmd
                    {
                        let Ok(location) = compat::resolve_node_location(&ctx.registry, node) else {
                            compat::emit_unresolved_node_request(
                                &ctx.events,
                                node,
                                session,
                                request,
                                "scan.node.compat resolve",
                            );
                            return;
                        };
                        let _ = tx.send(ScanCommand::ScanCompat {
                            location,
                            session,
                            pipeline,
                            load,
                            request,
                        });
                    }
                });
            }

            {
                let tx = watch_tx.clone();
                handlers.on("watch.node.compat", move |cmd, ctx| {
                    if let Command::WatchNodeCompat { node, session } = cmd {
                        let Ok(location) = compat::resolve_node_location(&ctx.registry, node) else {
                            compat::emit_unresolved_node_session(
                                &ctx.events,
                                node,
                                session,
                                "watch.node.compat resolve",
                            );
                            return;
                        };
                        let _ = tx.send(WatchCommand::Watch {
                            location,
                            session,
                            request: None,
                            event_mode: WatchEventMode::Compat { node },
                        });
                    }
                });
            }
            {
                let tx = watch_tx.clone();
                handlers.on("watch", move |cmd, _ctx| {
                    if let Command::Watch {
                        location,
                        session,
                        request,
                    } = cmd
                    {
                        let _ = tx.send(WatchCommand::Watch {
                            location,
                            session,
                            request: Some(request),
                            event_mode: WatchEventMode::Location,
                        });
                    }
                });
            }
            {
                let tx = watch_tx.clone();
                handlers.on("watch.node.remove.compat", move |cmd, ctx| {
                    if let Command::UnwatchNodeCompat { node } = cmd {
                        let Ok(location) = compat::resolve_node_location(&ctx.registry, node) else {
                            return;
                        };
                        let _ = tx.send(WatchCommand::Unwatch {
                            location,
                            scope: UnwatchScope::All,
                        });
                    }
                });
            }
            {
                let tx = watch_tx.clone();
                handlers.on("watch.remove", move |cmd, _ctx| {
                    if let Command::Unwatch { location, session } = cmd {
                        let _ = tx.send(WatchCommand::Unwatch {
                            location,
                            scope: UnwatchScope::Session(session),
                        });
                    }
                });
            }
            {
                let tx = watch_tx.clone();
                handlers.on("watch.session_remove", move |cmd, _ctx| {
                    if let Command::UnwatchSession(session) = cmd {
                        let _ = tx.send(WatchCommand::UnwatchSession(session));
                    }
                });
            }

            {
                let tx = preview_tx.clone();
                handlers.on("preview.load.node.compat", move |cmd, ctx| {
                    if let Command::LoadPreviewNodeCompat {
                        id,
                        options,
                        session,
                        request,
                    } = cmd
                    {
                        let Ok(location) = compat::resolve_node_location(&ctx.registry, id) else {
                            compat::emit_unresolved_node_request(
                                &ctx.events,
                                id,
                                session,
                                request,
                                "preview.load.node.compat resolve",
                            );
                            return;
                        };
                        let _ = tx.send(PreviewCommand::Generate {
                            location,
                            options,
                            event_mode: PreviewEventMode::Compat { node: id },
                            session,
                            request,
                        });
                    }
                });
            }
            {
                let tx = preview_tx.clone();
                handlers.on("preview.load", move |cmd, _ctx| {
                    if let Command::LoadPreview {
                        location,
                        options,
                        session,
                        request,
                    } = cmd
                    {
                        let _ = tx.send(PreviewCommand::Generate {
                            location,
                            options,
                            event_mode: PreviewEventMode::Location,
                            session,
                            request,
                        });
                    }
                });
            }
            {
                let tx = preview_tx.clone();
                handlers.on("preview.cancel", move |cmd, _ctx| {
                    if let Command::CancelPreview { session } = cmd {
                        let _ = tx.send(PreviewCommand::Cancel(session));
                    }
                });
            }
            {
                let tx = preview_tx.clone();
                handlers.on("metadata.load.node.compat", move |cmd, ctx| {
                    if let Command::LoadMetadataNodeCompat {
                        node,
                        session,
                        request,
                    } = cmd
                    {
                        let Ok(location) = compat::resolve_node_location(&ctx.registry, node) else {
                            compat::emit_unresolved_node_request(
                                &ctx.events,
                                node,
                                session,
                                request,
                                "metadata.load.node.compat resolve",
                            );
                            return;
                        };
                        let _ = tx.send(PreviewCommand::LoadMetadata {
                            location,
                            event_mode: PreviewEventMode::Compat { node },
                            session,
                            request,
                        });
                    }
                });
            }
            {
                let tx = preview_tx.clone();
                handlers.on("metadata.load", move |cmd, _ctx| {
                    if let Command::LoadMetadata {
                        location,
                        session,
                        request,
                    } = cmd
                    {
                        let _ = tx.send(PreviewCommand::LoadMetadata {
                            location,
                            event_mode: PreviewEventMode::Location,
                            session,
                            request,
                        });
                    }
                });
            }
            {
                let tx = preview_tx.clone();
                handlers.on("metadata.extended.node.compat", move |cmd, ctx| {
                    if let Command::LoadExtendedMetadataNodeCompat {
                        node,
                        session,
                        request,
                    } = cmd
                    {
                        let Ok(location) = compat::resolve_node_location(&ctx.registry, node) else {
                            compat::emit_unresolved_node_request(
                                &ctx.events,
                                node,
                                session,
                                request,
                                "metadata.extended.node.compat resolve",
                            );
                            return;
                        };
                        let _ = tx.send(PreviewCommand::LoadExtendedMetadata {
                            location,
                            event_mode: PreviewEventMode::Compat { node },
                            session,
                            request,
                        });
                    }
                });
            }
            {
                let tx = preview_tx.clone();
                handlers.on("metadata.extended", move |cmd, _ctx| {
                    if let Command::LoadExtendedMetadata {
                        location,
                        session,
                        request,
                    } = cmd
                    {
                        let _ = tx.send(PreviewCommand::LoadExtendedMetadata {
                            location,
                            event_mode: PreviewEventMode::Location,
                            session,
                            request,
                        });
                    }
                });
            }

            {
                let tx = ops_tx.clone();
                handlers.on("ops.copy.node.compat", move |cmd, ctx| {
                    if let Command::CopyNodeCompat {
                        sources,
                        destination,
                        session,
                        request,
                        operation,
                    } = cmd
                    {
                        let sources = match compat::resolve_node_locations(&ctx.registry, sources) {
                            Ok(sources) => sources,
                            Err(error) => {
                                send_or_warn(
                                    &ctx.events,
                                    Event::from_operation_error(error, session, request, operation),
                                    "operations: copy source compat resolve",
                                );
                                return;
                            }
                        };
                        let Ok(destination) =
                            compat::resolve_node_location(&ctx.registry, destination)
                        else {
                            compat::emit_unresolved_node_operation(
                                &ctx.events,
                                destination,
                                session,
                                request,
                                operation,
                                "operations: copy destination compat resolve",
                            );
                            return;
                        };
                        let _ = tx.send(OpsCommand::Copy {
                            sources,
                            destination,
                            event_mode: OperationEventMode::Compat,
                            session,
                            request,
                            operation,
                        });
                    }
                });
            }
            {
                let tx = ops_tx.clone();
                handlers.on("ops.copy", move |cmd, _ctx| {
                    if let Command::Copy {
                        sources,
                        destination,
                        session,
                        request,
                        operation,
                    } = cmd
                    {
                        let _ = tx.send(OpsCommand::Copy {
                            sources,
                            destination,
                            event_mode: OperationEventMode::Location,
                            session,
                            request,
                            operation,
                        });
                    }
                });
            }
            {
                let tx = ops_tx.clone();
                handlers.on("ops.move.node.compat", move |cmd, ctx| {
                    if let Command::MoveNodeCompat {
                        sources,
                        destination,
                        session,
                        request,
                        operation,
                    } = cmd
                    {
                        let sources = match compat::resolve_node_locations(&ctx.registry, sources) {
                            Ok(sources) => sources,
                            Err(error) => {
                                send_or_warn(
                                    &ctx.events,
                                    Event::from_operation_error(error, session, request, operation),
                                    "operations: move source compat resolve",
                                );
                                return;
                            }
                        };
                        let Ok(destination) =
                            compat::resolve_node_location(&ctx.registry, destination)
                        else {
                            compat::emit_unresolved_node_operation(
                                &ctx.events,
                                destination,
                                session,
                                request,
                                operation,
                                "operations: move destination compat resolve",
                            );
                            return;
                        };
                        let _ = tx.send(OpsCommand::Move {
                            sources,
                            destination,
                            event_mode: OperationEventMode::Compat,
                            session,
                            request,
                            operation,
                        });
                    }
                });
            }
            {
                let tx = ops_tx.clone();
                handlers.on("ops.move", move |cmd, _ctx| {
                    if let Command::Move {
                        sources,
                        destination,
                        session,
                        request,
                        operation,
                    } = cmd
                    {
                        let _ = tx.send(OpsCommand::Move {
                            sources,
                            destination,
                            event_mode: OperationEventMode::Location,
                            session,
                            request,
                            operation,
                        });
                    }
                });
            }
            {
                let tx = ops_tx.clone();
                handlers.on("ops.delete.node.compat", move |cmd, ctx| {
                    if let Command::DeleteNodeCompat {
                        nodes,
                        trash,
                        session,
                        request,
                        operation,
                    } = cmd
                    {
                        let targets = match compat::resolve_node_locations(&ctx.registry, nodes) {
                            Ok(targets) => targets,
                            Err(error) => {
                                send_or_warn(
                                    &ctx.events,
                                    Event::from_operation_error(error, session, request, operation),
                                    "operations: delete compat resolve",
                                );
                                return;
                            }
                        };
                        let _ = tx.send(OpsCommand::Delete {
                            targets,
                            trash,
                            event_mode: OperationEventMode::Compat,
                            session,
                            request,
                            operation,
                        });
                    }
                });
            }
            {
                let tx = ops_tx.clone();
                handlers.on("ops.delete", move |cmd, _ctx| {
                    if let Command::Delete {
                        locations,
                        trash,
                        session,
                        request,
                        operation,
                    } = cmd
                    {
                        let _ = tx.send(OpsCommand::Delete {
                            targets: locations,
                            trash,
                            event_mode: OperationEventMode::Location,
                            session,
                            request,
                            operation,
                        });
                    }
                });
            }
            {
                let tx = ops_tx.clone();
                handlers.on("ops.rename.node.compat", move |cmd, ctx| {
                    if let Command::RenameNodeCompat {
                        node,
                        new_name,
                        session,
                        request,
                        operation,
                    } = cmd
                    {
                        let Ok(source) = compat::resolve_node_location(&ctx.registry, node) else {
                            compat::emit_unresolved_node_operation(
                                &ctx.events,
                                node,
                                session,
                                request,
                                operation,
                                "operations: rename compat resolve",
                            );
                            return;
                        };
                        let _ = tx.send(OpsCommand::Rename {
                            source,
                            new_name,
                            event_mode: OperationEventMode::Compat,
                            session,
                            request,
                            operation,
                        });
                    }
                });
            }
            {
                let tx = ops_tx.clone();
                handlers.on("ops.rename", move |cmd, _ctx| {
                    if let Command::Rename {
                        location,
                        new_name,
                        session,
                        request,
                        operation,
                    } = cmd
                    {
                        let _ = tx.send(OpsCommand::Rename {
                            source: location,
                            new_name,
                            event_mode: OperationEventMode::Location,
                            session,
                            request,
                            operation,
                        });
                    }
                });
            }
            {
                let tx = ops_tx.clone();
                handlers.on("ops.create_folder.node.compat", move |cmd, ctx| {
                    if let Command::CreateFolderNodeCompat {
                        parent,
                        name,
                        session,
                        request,
                        operation,
                    } = cmd
                    {
                        let Ok(parent) = compat::resolve_node_location(&ctx.registry, parent) else {
                            compat::emit_unresolved_node_operation(
                                &ctx.events,
                                parent,
                                session,
                                request,
                                operation,
                                "operations: create folder compat resolve",
                            );
                            return;
                        };
                        let _ = tx.send(OpsCommand::CreateFolder {
                            parent,
                            name,
                            event_mode: OperationEventMode::Compat,
                            session,
                            request,
                            operation,
                        });
                    }
                });
            }
            {
                let tx = ops_tx.clone();
                handlers.on("ops.create_folder", move |cmd, _ctx| {
                    if let Command::CreateFolder {
                        parent,
                        name,
                        session,
                        request,
                        operation,
                    } = cmd
                    {
                        let _ = tx.send(OpsCommand::CreateFolder {
                            parent,
                            name,
                            event_mode: OperationEventMode::Location,
                            session,
                            request,
                            operation,
                        });
                    }
                });
            }
            {
                let tx = ops_tx.clone();
                handlers.on("ops.create_file.node.compat", move |cmd, ctx| {
                    if let Command::CreateFileNodeCompat {
                        parent,
                        name,
                        session,
                        request,
                        operation,
                    } = cmd
                    {
                        let Ok(parent) = compat::resolve_node_location(&ctx.registry, parent) else {
                            compat::emit_unresolved_node_operation(
                                &ctx.events,
                                parent,
                                session,
                                request,
                                operation,
                                "operations: create file compat resolve",
                            );
                            return;
                        };
                        let _ = tx.send(OpsCommand::CreateFile {
                            parent,
                            name,
                            event_mode: OperationEventMode::Compat,
                            session,
                            request,
                            operation,
                        });
                    }
                });
            }
            {
                let tx = ops_tx.clone();
                handlers.on("ops.create_file", move |cmd, _ctx| {
                    if let Command::CreateFile {
                        parent,
                        name,
                        session,
                        request,
                        operation,
                    } = cmd
                    {
                        let _ = tx.send(OpsCommand::CreateFile {
                            parent,
                            name,
                            event_mode: OperationEventMode::Location,
                            session,
                            request,
                            operation,
                        });
                    }
                });
            }
            {
                let tx = ops_tx.clone();
                handlers.on("ops.cancel", move |cmd, _ctx| {
                    if let Command::CancelOperation { session, operation } = cmd {
                        let _ = tx.send(OpsCommand::CancelOperation { session, operation });
                    }
                });
            }

            {
                let tx = watch_tx.clone();
                handlers.on_session_destroy(move |session, _ctx| {
                    let _ = tx.send(WatchCommand::UnwatchSession(session));
                });
            }
            {
                let tx = nav_tx.clone();
                handlers.on_session_destroy(move |session, _ctx| {
                    let _ = tx.send(NavCommand::RemoveSession(session));
                });
            }

    }
