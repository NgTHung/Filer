#[test]
fn test_pipeline_config_default_is_pageable() {
    assert!(PipelineConfig::default().is_pageable());
}

#[test]
fn test_pipeline_config_sort_filter_group_are_not_pageable() {
    assert!(!PipelineConfig::with_default_sort().is_pageable());
    assert!(
        !PipelineConfig::default()
            .filter(FilterConfig::only_extensions(vec!["rs".into()]))
            .is_pageable()
    );
    assert!(
        !PipelineConfig::default()
            .group_by(ConfigGroupBy::Extension)
            .is_pageable()
    );
}

#[test]
fn test_pipeline_paging_mode_classifies_provider_pages() {
    assert_eq!(
        PipelineConfig::default().paging_mode(),
        PipelinePagingMode::ProviderPage
    );
}

#[test]
fn test_pipeline_paging_mode_classifies_filter_only_pages() {
    assert_eq!(
        PipelineConfig::default().show_hidden(false).paging_mode(),
        PipelinePagingMode::FilteredPage
    );
    assert_eq!(
        PipelineConfig::default()
            .filter(FilterConfig::only_extensions(vec!["rs".into()]))
            .paging_mode(),
        PipelinePagingMode::FilteredPage
    );
    assert_eq!(
        PipelineConfig::default()
            .filter(FilterConfig::exclude_extensions(vec!["tmp".into()]))
            .paging_mode(),
        PipelinePagingMode::FilteredPage
    );
}

#[test]
fn test_pipeline_paging_mode_keeps_order_changing_or_unsupported_filters_snapshot_only() {
    assert_eq!(
        PipelineConfig::with_default_sort().paging_mode(),
        PipelinePagingMode::PipelinePage
    );
    assert_eq!(
        PipelineConfig::default()
            .group_by(ConfigGroupBy::Extension)
            .paging_mode(),
        PipelinePagingMode::PipelinePage
    );

    let mut filter = FilterConfig {
        min_size: Some(10),
        ..Default::default()
    };
    assert_eq!(
        PipelineConfig::default()
            .filter(filter.clone())
            .paging_mode(),
        PipelinePagingMode::SnapshotOnly
    );

    filter.min_size = None;
    filter.name_pattern = Some("*.rs".into());
    assert_eq!(
        PipelineConfig::default().filter(filter).paging_mode(),
        PipelinePagingMode::SnapshotOnly
    );
}

