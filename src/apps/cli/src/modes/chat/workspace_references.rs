impl ChatMode {
    fn refresh_workspace_reference_search(&mut self, chat_view: &mut ChatView) -> bool {
        let query = chat_view.current_workspace_reference_query();
        let query_text = query.as_ref().map(|query| query.path_query.clone());
        if query_text == self.last_workspace_reference_query {
            if chat_view.workspace_reference_popup_visible() {
                chat_view.set_workspace_reference_query(query);
            }
            return false;
        }

        if let Some(pending) = self.pending_workspace_reference_search.take() {
            pending.handle.abort();
        }
        self.workspace_reference_search_generation =
            self.workspace_reference_search_generation.wrapping_add(1);
        self.last_workspace_reference_query = query_text.clone();
        chat_view.set_workspace_reference_query(query);

        let Some(query) = query_text else {
            return true;
        };
        let generation = self.workspace_reference_search_generation;
        let agent = self.agent.clone();
        let search_query = query.clone();
        let handle = tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(80)).await;
            agent
                .search_workspace_references(search_query)
                .await
                .map_err(|error| error.to_string())
        });
        self.pending_workspace_reference_search = Some(PendingWorkspaceReferenceSearch {
            generation,
            query,
            handle,
        });
        true
    }

    fn poll_workspace_reference_search(&mut self, chat_view: &mut ChatView) -> bool {
        let Some(pending) = self.pending_workspace_reference_search.as_ref() else {
            return false;
        };
        if !pending.handle.is_finished() {
            return false;
        }
        let pending = self
            .pending_workspace_reference_search
            .take()
            .expect("workspace reference search was checked above");
        let current = self.last_workspace_reference_query.as_deref();
        if pending.generation != self.workspace_reference_search_generation
            || current != Some(pending.query.as_str())
        {
            return false;
        }
        match tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(pending.handle)
        }) {
            Ok(Ok(result)) => {
                chat_view.set_workspace_reference_results(result.entries);
            }
            Ok(Err(error)) => {
                chat_view.set_workspace_reference_results(Vec::new());
                chat_view.set_status(Some(format!(
                    "Workspace reference search failed: {error}"
                )));
            }
            Err(error) if error.is_cancelled() => return false,
            Err(error) => {
                chat_view.set_workspace_reference_results(Vec::new());
                chat_view.set_status(Some(format!(
                    "Workspace reference search stopped: {error}"
                )));
            }
        }
        true
    }
}
