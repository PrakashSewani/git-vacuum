use std::sync::Arc;

use git_vacuum_core::{DashboardStats, DbError};

use crate::Services;

pub async fn compute(services: Arc<Services>) -> Result<DashboardStats, DbError> {
    services.db.get_dashboard_stats()
}
