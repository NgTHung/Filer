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
                        root,
                        session,
                        request,
                    } = cmd
                    {
                        match crate::model::query::SearchQuery::parse(&query) {
                            Ok(query) => {
                                let _ = tx.send(SearchCommand::Search {
                                    query,
                                    root,
                                    session,
                                    request,
                                });
                            }
                            Err(error) => {
                                let _ = ctx.events.send(Event::from_request_error(
                                    CoreError::invalid_input(format!(
                                        "Invalid search query: {error}"
                                    )),
                                    session,
                                    request,
                                ));
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
                                let _ = tx.send(SearchCommand::SearchPath {
                                    query,
                                    root,
                                    session,
                                    request,
                                });
                            }
                            Err(error) => {
                                let _ = ctx.events.send(Event::from_request_error(
                                    CoreError::invalid_input(format!(
                                        "Invalid search query: {error}"
                                    )),
                                    session,
                                    request,
                                ));
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
                                let _ = tx.send(SearchCommand::SearchLocation {
                                    query,
                                    root,
                                    session,
                                    request,
                                });
                            }
                            Err(error) => {
                                let _ = ctx.events.send(Event::from_request_error(
                                    CoreError::invalid_input(format!(
                                        "Invalid search query: {error}"
                                    )),
                                    session,
                                    request,
                                ));
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
                        let _ = tx.send(ScanCommand::Scan {
                            path,
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
                handlers.on("scan.node.compat", move |cmd, _ctx| {
                    if let Command::ScanNodeCompat {
                        node,
                        session,
                        pipeline,
                        load,
                        request,
                    } = cmd
                    {
                        let _ = tx.send(ScanCommand::ScanNode {
                            node,
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
                handlers.on("watch.node.compat", move |cmd, _ctx| {
                    if let Command::WatchNodeCompat { node, session } = cmd {
                        let _ = tx.send(WatchCommand::Watch(node, session));
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
                        let _ = tx.send(WatchCommand::WatchLocation {
                            location,
                            session,
                            request,
                        });
                    }
                });
            }
            {
                let tx = watch_tx.clone();
                handlers.on("watch.node.remove.compat", move |cmd, _ctx| {
                    if let Command::UnwatchNodeCompat { node } = cmd {
                        let _ = tx.send(WatchCommand::Unwatch(node));
                    }
                });
            }
            {
                let tx = watch_tx.clone();
                handlers.on("watch.remove", move |cmd, _ctx| {
                    if let Command::Unwatch { location, session } = cmd {
                        let _ = tx.send(WatchCommand::UnwatchLocation { location, session });
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
                handlers.on("preview.load.node.compat", move |cmd, _ctx| {
                    if let Command::LoadPreviewNodeCompat {
                        id,
                        options,
                        session,
                        request,
                    } = cmd
                    {
                        let _ = tx.send(PreviewCommand::Generate {
                            path: id,
                            options,
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
                        let _ = tx.send(PreviewCommand::GenerateLocation {
                            location,
                            options,
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
                handlers.on("metadata.load.node.compat", move |cmd, _ctx| {
                    if let Command::LoadMetadataNodeCompat {
                        node,
                        session,
                        request,
                    } = cmd
                    {
                        let _ = tx.send(PreviewCommand::LoadMetadata(node, session, request));
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
                        let _ = tx.send(PreviewCommand::LoadMetadataLocation(
                            location, session, request,
                        ));
                    }
                });
            }
            {
                let tx = preview_tx.clone();
                handlers.on("metadata.extended.node.compat", move |cmd, _ctx| {
                    if let Command::LoadExtendedMetadataNodeCompat {
                        node,
                        session,
                        request,
                    } = cmd
                    {
                        let _ =
                            tx.send(PreviewCommand::LoadExtendedMetadata(node, session, request));
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
                        let _ = tx.send(PreviewCommand::LoadExtendedMetadataLocation(
                            location, session, request,
                        ));
                    }
                });
            }

            {
                let tx = ops_tx.clone();
                handlers.on("ops.copy.node.compat", move |cmd, _ctx| {
                    if let Command::CopyNodeCompat {
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
                        let _ = tx.send(OpsCommand::CopyLocation {
                            sources,
                            destination,
                            session,
                            request,
                            operation,
                        });
                    }
                });
            }
            {
                let tx = ops_tx.clone();
                handlers.on("ops.move.node.compat", move |cmd, _ctx| {
                    if let Command::MoveNodeCompat {
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
                        let _ = tx.send(OpsCommand::MoveLocation {
                            sources,
                            destination,
                            session,
                            request,
                            operation,
                        });
                    }
                });
            }
            {
                let tx = ops_tx.clone();
                handlers.on("ops.delete.node.compat", move |cmd, _ctx| {
                    if let Command::DeleteNodeCompat {
                        nodes,
                        trash,
                        session,
                        request,
                        operation,
                    } = cmd
                    {
                        let _ = tx.send(OpsCommand::Delete {
                            targets: nodes,
                            trash,
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
                        let _ = tx.send(OpsCommand::DeleteLocation {
                            targets: locations,
                            trash,
                            session,
                            request,
                            operation,
                        });
                    }
                });
            }
            {
                let tx = ops_tx.clone();
                handlers.on("ops.rename.node.compat", move |cmd, _ctx| {
                    if let Command::RenameNodeCompat {
                        node,
                        new_name,
                        session,
                        request,
                        operation,
                    } = cmd
                    {
                        let _ = tx.send(OpsCommand::Rename {
                            source: node,
                            new_name,
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
                        let _ = tx.send(OpsCommand::RenameLocation {
                            source: location,
                            new_name,
                            session,
                            request,
                            operation,
                        });
                    }
                });
            }
            {
                let tx = ops_tx.clone();
                handlers.on("ops.create_folder.node.compat", move |cmd, _ctx| {
                    if let Command::CreateFolderNodeCompat {
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
                        let _ = tx.send(OpsCommand::CreateFolderLocation {
                            parent,
                            name,
                            session,
                            request,
                            operation,
                        });
                    }
                });
            }
            {
                let tx = ops_tx.clone();
                handlers.on("ops.create_file.node.compat", move |cmd, _ctx| {
                    if let Command::CreateFileNodeCompat {
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
                        let _ = tx.send(OpsCommand::CreateFileLocation {
                            parent,
                            name,
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
