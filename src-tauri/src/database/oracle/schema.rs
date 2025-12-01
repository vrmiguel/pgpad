use std::collections::HashSet;
use std::sync::{Arc, Mutex};

use crate::database::types::{ColumnInfo, DatabaseSchema, TableInfo};
use crate::Error;

pub async fn get_database_schema(conn: Arc<Mutex<oracle::Connection>>) -> Result<DatabaseSchema, Error> {
    tauri::async_runtime::spawn_blocking(move || {
        let conn = conn.lock().map_err(|e| Error::Any(anyhow::anyhow!(format!("Mutex poisoned: {}", e))))?;

        let mut tables = Vec::new();
        let mut unique_columns_set = HashSet::new();
        let mut schemas_set = HashSet::new();

        let rows = conn
            .query("SELECT owner, table_name FROM all_tables WHERE owner NOT IN ('SYS','SYSTEM') ORDER BY owner, table_name", &[])
            .map_err(|e| Error::Any(anyhow::anyhow!(e)))?;
        for row_res in rows {
            let row = row_res.map_err(|e| Error::Any(anyhow::anyhow!(e)))?;
            let owner: String = row.get(0).map_err(|e| Error::Any(anyhow::anyhow!(e)))?;
            let table_name: String = row.get(1).map_err(|e| Error::Any(anyhow::anyhow!(e)))?;

            schemas_set.insert(owner.clone());

            let col_rows = conn
                .query(
                "SELECT column_name, data_type, nullable, data_default FROM all_tab_columns WHERE owner = :1 AND table_name = :2 ORDER BY column_id",
                &[&owner, &table_name],
            )
                .map_err(|e| Error::Any(anyhow::anyhow!(e)))?;

            let mut columns = Vec::new();
            for crow_res in col_rows {
                let crow = crow_res.map_err(|e| Error::Any(anyhow::anyhow!(e)))?;
                let column_name: String = crow.get(0).map_err(|e| Error::Any(anyhow::anyhow!(e)))?;
                let data_type: String = crow.get(1).map_err(|e| Error::Any(anyhow::anyhow!(e)))?;
                let nullable: String = crow.get(2).map_err(|e| Error::Any(anyhow::anyhow!(e)))?;
                let default_value: Option<String> = crow.get(3).map_err(|e| Error::Any(anyhow::anyhow!(e)))?;
                unique_columns_set.insert(column_name.clone());
                columns.push(ColumnInfo { name: column_name, data_type, is_nullable: nullable == "Y", default_value });
            }

            tables.push(TableInfo { name: table_name, schema: owner, columns });
        }

        // Include views
        let view_rows = conn
            .query("SELECT owner, view_name FROM all_views WHERE owner NOT IN ('SYS','SYSTEM') ORDER BY owner, view_name", &[])
            .map_err(|e| Error::Any(anyhow::anyhow!(e)))?;
        for row_res in view_rows {
            let row = row_res.map_err(|e| Error::Any(anyhow::anyhow!(e)))?;
            let owner: String = row.get(0).map_err(|e| Error::Any(anyhow::anyhow!(e)))?;
            let view_name: String = row.get(1).map_err(|e| Error::Any(anyhow::anyhow!(e)))?;

            schemas_set.insert(owner.clone());

            let col_rows = conn
                .query(
                "SELECT column_name, data_type, nullable, data_default FROM all_tab_columns WHERE owner = :1 AND table_name = :2 ORDER BY column_id",
                &[&owner, &view_name],
            )
                .map_err(|e| Error::Any(anyhow::anyhow!(e)))?;

            let mut columns = Vec::new();
            for crow_res in col_rows {
                let crow = crow_res.map_err(|e| Error::Any(anyhow::anyhow!(e)))?;
                let column_name: String = crow.get(0).map_err(|e| Error::Any(anyhow::anyhow!(e)))?;
                let data_type: String = crow.get(1).map_err(|e| Error::Any(anyhow::anyhow!(e)))?;
                let nullable: String = crow.get(2).map_err(|e| Error::Any(anyhow::anyhow!(e)))?;
                let default_value: Option<String> = crow.get(3).map_err(|e| Error::Any(anyhow::anyhow!(e)))?;
                unique_columns_set.insert(column_name.clone());
                columns.push(ColumnInfo { name: column_name, data_type, is_nullable: nullable == "Y", default_value });
            }

            tables.push(TableInfo { name: view_name, schema: owner, columns });
        }

        // Include materialized views
        let mv_rows = conn
            .query(
                "SELECT owner, mview_name, query, refresh_mode, refresh_method, build_mode, TO_CHAR(last_refresh_date, 'YYYY-MM-DD HH24:MI:SS'), last_refresh_type, TO_CHAR(next, 'YYYY-MM-DD HH24:MI:SS') FROM all_mviews WHERE owner NOT IN ('SYS','SYSTEM') ORDER BY owner, mview_name",
                &[],
            )
            .map_err(|e| Error::Any(anyhow::anyhow!(e)))?;
        for row_res in mv_rows {
            let row = row_res.map_err(|e| Error::Any(anyhow::anyhow!(e)))?;
            let owner: String = row.get(0).map_err(|e| Error::Any(anyhow::anyhow!(e)))?;
            let mv_name: String = row.get(1).map_err(|e| Error::Any(anyhow::anyhow!(e)))?;
            let mv_query: Option<String> = row.get(2).map_err(|e| Error::Any(anyhow::anyhow!(e)))?;
            let refresh_mode: Option<String> = row.get(3).ok();
            let refresh_method: Option<String> = row.get(4).ok();
            let build_mode: Option<String> = row.get(5).ok();
            let last_refresh_date: Option<String> = row.get(6).ok();
            let last_refresh_type: Option<String> = row.get(7).ok();
            let next_refresh: Option<String> = row.get(8).ok();
            schemas_set.insert(owner.clone());
            let col_rows = conn
                .query(
                    "SELECT column_name, data_type, nullable, data_default FROM all_tab_columns WHERE owner = :1 AND table_name = :2 ORDER BY column_id",
                    &[&owner, &mv_name],
                )
                .map_err(|e| Error::Any(anyhow::anyhow!(e)))?;
            let mut columns = Vec::new();
            if let Some(q) = mv_query { columns.push(ColumnInfo { name: "__MV_QUERY__".into(), data_type: "".into(), is_nullable: false, default_value: Some(q) }); }
            if let Some(v) = refresh_mode { columns.push(ColumnInfo { name: "__MV_REFRESH_MODE__".into(), data_type: v, is_nullable: false, default_value: None }); }
            if let Some(v) = refresh_method { columns.push(ColumnInfo { name: "__MV_REFRESH_METHOD__".into(), data_type: v, is_nullable: false, default_value: None }); }
            if let Some(v) = build_mode { columns.push(ColumnInfo { name: "__MV_BUILD_MODE__".into(), data_type: v, is_nullable: false, default_value: None }); }
            if let Some(v) = last_refresh_date { columns.push(ColumnInfo { name: "__MV_LAST_REFRESH_DATE__".into(), data_type: v, is_nullable: false, default_value: None }); }
            if let Some(v) = last_refresh_type { columns.push(ColumnInfo { name: "__MV_LAST_REFRESH_TYPE__".into(), data_type: v, is_nullable: false, default_value: None }); }
            if let Some(v) = next_refresh { columns.push(ColumnInfo { name: "__MV_NEXT_REFRESH__".into(), data_type: v, is_nullable: false, default_value: None }); }

            // Scheduler jobs possibly controlling refresh
            if let Ok(mut jobs) = conn.query(
                "SELECT job_name, enabled, repeat_interval \
                 FROM all_scheduler_jobs \
                 WHERE UPPER(job_action) LIKE '%' || UPPER(:1) || '%'",
                &[&mv_name],
            ) {
                let mut job_summaries = Vec::new();
                while let Some(Ok(j)) = jobs.next() {
                    let jname: String = j.get(0).unwrap_or_default();
                    let enabled: String = j.get(1).unwrap_or_default();
                    let repeat: Option<String> = j.get(2).ok();
                    let summary = match repeat { Some(r) => format!("{}[{} {}]", jname, enabled, r), None => format!("{}[{}]", jname, enabled) };
                    job_summaries.push(summary);
                }
                if !job_summaries.is_empty() {
                    columns.push(ColumnInfo { name: "__MV_SCHED_JOBS__".into(), data_type: job_summaries.join(", "), is_nullable: false, default_value: None });
                }
            }

            // Last run details for jobs referencing this MV by name
            if let Ok(mut runs) = conn.query(
                "SELECT job_name, status, TO_CHAR(actual_start_date, 'YYYY-MM-DD HH24:MI:SS'), run_duration \
                 FROM all_scheduler_job_run_details \
                 WHERE job_name IN (SELECT job_name FROM all_scheduler_jobs WHERE UPPER(job_action) LIKE '%' || UPPER(:1) || '%') \
                 ORDER BY actual_start_date DESC",
                &[&mv_name],
            ) {
                let mut summaries = Vec::new();
                // capture up to a few entries
                let mut count = 0usize;
                while count < 3 {
                    match runs.next() {
                        Some(Ok(r)) => {
                            let jname: String = r.get(0).unwrap_or_default();
                            let status: String = r.get(1).unwrap_or_default();
                            let started: Option<String> = r.get(2).ok();
                            let dur: Option<String> = r.get(3).ok();
                            let summary = match (started, dur) {
                                (Some(s), Some(d)) => format!("{}:{}@{} ({})", jname, status, s, d),
                                (Some(s), None) => format!("{}:{}@{}", jname, status, s),
                                _ => format!("{}:{}", jname, status),
                            };
                            summaries.push(summary);
                            count += 1;
                        }
                        _ => break,
                    }
                }
                if !summaries.is_empty() {
                    columns.push(ColumnInfo { name: "__MV_SCHED_LAST_RUN__".into(), data_type: summaries.join(" | "), is_nullable: false, default_value: None });
                }
            }

            // Last error details (guarded): pick most recent non-success run and show req_start_date and truncated additional_info
            if let Ok(mut runs) = conn.query(
                "SELECT job_name, status, TO_CHAR(req_start_date, 'YYYY-MM-DD HH24:MI:SS'), SUBSTR(additional_info, 1, 256) \
                 FROM all_scheduler_job_run_details \
                 WHERE job_name IN (SELECT job_name FROM all_scheduler_jobs WHERE UPPER(job_action) LIKE '%' || UPPER(:1) || '%') \
                   AND UPPER(status) <> 'SUCCEEDED' \
                 ORDER BY req_start_date DESC",
                &[&mv_name],
            ) {
                if let Some(Ok(r)) = runs.next() {
                    let jname: String = r.get(0).unwrap_or_default();
                    let status: String = r.get(1).unwrap_or_default();
                    let req: Option<String> = r.get(2).ok();
                    let info: Option<String> = r.get(3).ok();
                    let summary = match (req, info) {
                        (Some(s), Some(i)) => format!("{}:{}@{} {}", jname, status, s, i),
                        (Some(s), None) => format!("{}:{}@{}", jname, status, s),
                        _ => format!("{}:{}", jname, status),
                    };
                    columns.push(ColumnInfo { name: "__MV_SCHED_LAST_ERROR__".into(), data_type: summary, is_nullable: false, default_value: None });
                }
            }

            // Jobs that explicitly call DBMS_MVIEW.REFRESH for this MV (owner-qualified)
            if let Ok(mut jobs) = conn.query(
                "SELECT owner, job_name, enabled, repeat_interval \
                 FROM all_scheduler_jobs \
                 WHERE UPPER(job_action) LIKE '%DBMS_MVIEW.REFRESH%' \
                   AND UPPER(job_action) LIKE '%' || UPPER(:1) || '%' \
                   AND UPPER(job_action) LIKE '%' || UPPER(:2) || '%'",
                &[&mv_name, &owner],
            ) {
                let mut job_summaries = Vec::new();
                while let Some(Ok(j)) = jobs.next() {
                    let jowner: String = j.get(0).unwrap_or_default();
                    let jname: String = j.get(1).unwrap_or_default();
                    let enabled: String = j.get(2).unwrap_or_default();
                    let repeat: Option<String> = j.get(3).ok();
                    let summary = match repeat { Some(r) => format!("{}.{}/{} {}", jowner, jname, enabled, r), None => format!("{}.{}/{}", jowner, jname, enabled) };
                    job_summaries.push(summary);
                }
                if !job_summaries.is_empty() {
                    columns.push(ColumnInfo { name: "__MV_SCHED_REFRESH_JOBS__".into(), data_type: job_summaries.join(", "), is_nullable: false, default_value: None });
                }
            }

            // Refresh group membership (attempt USER_ views first, then DBA_ views)
            let mut groups: Vec<String> = Vec::new();
            // USER_ views
            if let Ok(mut q) = conn.query(
                "SELECT r.name \
                 FROM user_refresh r \
                 JOIN user_refresh_children c ON r.rowid = c.rowid \
                 WHERE c.owner = :1 AND c.name = :2",
                &[&owner, &mv_name],
            ) {
                while let Some(Ok(row)) = q.next() {
                    if let Ok(gname) = row.get::<usize, String>(0) { groups.push(gname); }
                }
            }
            // DBA_ views if USER_ yielded nothing or not accessible
            if groups.is_empty() {
                if let Ok(mut q) = conn.query(
                    "SELECT r.name \
                     FROM dba_refresh r \
                     JOIN dba_refresh_children c ON r.rowid = c.rowid \
                     WHERE c.owner = :1 AND c.name = :2",
                    &[&owner, &mv_name],
                ) {
                    while let Some(Ok(row)) = q.next() {
                        if let Ok(gname) = row.get::<usize, String>(0) { groups.push(gname); }
                    }
                }
            }
            if !groups.is_empty() {
                columns.push(ColumnInfo { name: "__MV_REFRESH_GROUPS__".into(), data_type: groups.join(", "), is_nullable: false, default_value: None });
            }
            for crow_res in col_rows {
                let crow = crow_res.map_err(|e| Error::Any(anyhow::anyhow!(e)))?;
                let column_name: String = crow.get(0).map_err(|e| Error::Any(anyhow::anyhow!(e)))?;
                let data_type: String = crow.get(1).map_err(|e| Error::Any(anyhow::anyhow!(e)))?;
                let nullable: String = crow.get(2).map_err(|e| Error::Any(anyhow::anyhow!(e)))?;
                let default_value: Option<String> = crow.get(3).map_err(|e| Error::Any(anyhow::anyhow!(e)))?;
                unique_columns_set.insert(column_name.clone());
                columns.push(ColumnInfo { name: column_name, data_type, is_nullable: nullable == "Y", default_value });
            }
            tables.push(TableInfo { name: mv_name, schema: owner, columns });
        }

        // Include indexes
        let idx_rows = conn
            .query(
                "SELECT owner, index_name, index_type, uniqueness, table_owner, table_name, partitioned, compression, visibility 
                 FROM all_indexes 
                 WHERE owner NOT IN ('SYS','SYSTEM') 
                 ORDER BY owner, index_name",
                &[],
            )
            .map_err(|e| Error::Any(anyhow::anyhow!(e)))?;
        for row_res in idx_rows {
            let row = row_res.map_err(|e| Error::Any(anyhow::anyhow!(e)))?;
            let owner: String = row.get(0).map_err(|e| Error::Any(anyhow::anyhow!(e)))?;
            let index_name: String = row.get(1).map_err(|e| Error::Any(anyhow::anyhow!(e)))?;
            let index_type: String = row.get(2).map_err(|e| Error::Any(anyhow::anyhow!(e)))?;
            let uniqueness: String = row.get(3).map_err(|e| Error::Any(anyhow::anyhow!(e)))?;
            let table_owner: String = row.get(4).map_err(|e| Error::Any(anyhow::anyhow!(e)))?;
            let table_name: String = row.get(5).map_err(|e| Error::Any(anyhow::anyhow!(e)))?;
            let partitioned: String = row.get(6).map_err(|e| Error::Any(anyhow::anyhow!(e)))?;
            let compression: String = row.get(7).map_err(|e| Error::Any(anyhow::anyhow!(e)))?;
            let visibility: String = row.get(8).map_err(|e| Error::Any(anyhow::anyhow!(e)))?;
            schemas_set.insert(owner.clone());
            let col_rows = conn
                .query("SELECT column_name FROM all_ind_columns WHERE index_owner = :1 AND index_name = :2 ORDER BY column_position",
                       &[&owner, &index_name])
                .map_err(|e| Error::Any(anyhow::anyhow!(e)))?;
            let mut columns = vec![
                ColumnInfo { name: "__INDEX_TYPE__".into(), data_type: index_type, is_nullable: false, default_value: None },
                ColumnInfo { name: "__INDEX_UNIQUENESS__".into(), data_type: uniqueness, is_nullable: false, default_value: None },
                ColumnInfo { name: "__INDEX_TABLE__".into(), data_type: format!("{}.{}", table_owner, table_name), is_nullable: false, default_value: None },
                ColumnInfo { name: "__INDEX_PARTITIONED__".into(), data_type: partitioned, is_nullable: false, default_value: None },
                ColumnInfo { name: "__INDEX_COMPRESSION__".into(), data_type: compression, is_nullable: false, default_value: None },
                ColumnInfo { name: "__INDEX_VISIBILITY__".into(), data_type: visibility, is_nullable: false, default_value: None },
            ];

            // Partition-level visibility/compression summary
            let part_rows = conn
                .query(
                    "SELECT partition_name, visibility, compression, prefix_length 
                     FROM all_ind_partitions 
                     WHERE index_owner = :1 AND index_name = :2 
                     ORDER BY partition_position",
                    &[&owner, &index_name],
                )
                .map_err(|e| Error::Any(anyhow::anyhow!(e)))?;
            let mut part_vis = String::new();
            let mut part_comp = String::new();
            for pres in part_rows {
                let prow = pres.map_err(|e| Error::Any(anyhow::anyhow!(e)))?;
                let pname: String = prow.get(0).map_err(|e| Error::Any(anyhow::anyhow!(e)))?;
                let pvis: String = prow.get(1).map_err(|e| Error::Any(anyhow::anyhow!(e)))?;
                let pcomp: String = prow.get(2).map_err(|e| Error::Any(anyhow::anyhow!(e)))?;
                let ppre: Option<i64> = prow.get(3).map_err(|e| Error::Any(anyhow::anyhow!(e)))?;
                if !part_vis.is_empty() { part_vis.push_str(", "); }
                part_vis.push_str(&format!("{}={}", pname, pvis));
                if !part_comp.is_empty() { part_comp.push_str(", "); }
                part_comp.push_str(&format!("{}={}", pname, match ppre { Some(n) => format!("{}({})", pcomp, n), None => pcomp.clone() }));
            }
            if !part_vis.is_empty() { columns.push(ColumnInfo { name: "__INDEX_PARTITION_VIS__".into(), data_type: part_vis, is_nullable: false, default_value: None }); }
            if !part_comp.is_empty() { columns.push(ColumnInfo { name: "__INDEX_PARTITION_COMP__".into(), data_type: part_comp, is_nullable: false, default_value: None }); }

            for crow_res in col_rows {
                let crow = crow_res.map_err(|e| Error::Any(anyhow::anyhow!(e)))?;
                let column_name: String = crow.get(0).map_err(|e| Error::Any(anyhow::anyhow!(e)))?;
                unique_columns_set.insert(column_name.clone());
                columns.push(ColumnInfo { name: column_name, data_type: "".into(), is_nullable: true, default_value: None });
            }
            tables.push(TableInfo { name: index_name, schema: owner, columns });
        }

        // Include constraints
        let cons_rows = conn
            .query("SELECT owner, constraint_name, constraint_type, search_condition, deferrable, validated FROM all_constraints WHERE owner NOT IN ('SYS','SYSTEM') ORDER BY owner, constraint_name", &[])
            .map_err(|e| Error::Any(anyhow::anyhow!(e)))?;
        for row_res in cons_rows {
            let row = row_res.map_err(|e| Error::Any(anyhow::anyhow!(e)))?;
            let owner: String = row.get(0).map_err(|e| Error::Any(anyhow::anyhow!(e)))?;
            let constraint_name: String = row.get(1).map_err(|e| Error::Any(anyhow::anyhow!(e)))?;
            let constraint_type: String = row.get(2).map_err(|e| Error::Any(anyhow::anyhow!(e)))?;
            let search_condition: Option<String> = row.get(3).map_err(|e| Error::Any(anyhow::anyhow!(e)))?;
            let deferrable: String = row.get(4).map_err(|e| Error::Any(anyhow::anyhow!(e)))?;
            let validated: String = row.get(5).map_err(|e| Error::Any(anyhow::anyhow!(e)))?;
            schemas_set.insert(owner.clone());
            let col_rows = conn
                .query("SELECT column_name FROM all_cons_columns WHERE owner = :1 AND constraint_name = :2 ORDER BY position",
                       &[&owner, &constraint_name])
                .map_err(|e| Error::Any(anyhow::anyhow!(e)))?;
            let mut columns = Vec::new();
            columns.push(ColumnInfo { name: "__CONSTRAINT_TYPE__".into(), data_type: describe_constraint(&constraint_type), is_nullable: false, default_value: None });
            if let Some(cond) = search_condition { columns.push(ColumnInfo { name: "__SEARCH_CONDITION__".into(), data_type: "".into(), is_nullable: false, default_value: Some(cond) }); }
            columns.push(ColumnInfo { name: "__DEFERRABLE__".into(), data_type: deferrable, is_nullable: false, default_value: None });
            columns.push(ColumnInfo { name: "__VALIDATED__".into(), data_type: validated, is_nullable: false, default_value: None });
            for crow_res in col_rows {
                let crow = crow_res.map_err(|e| Error::Any(anyhow::anyhow!(e)))?;
                let column_name: String = crow.get(0).map_err(|e| Error::Any(anyhow::anyhow!(e)))?;
                unique_columns_set.insert(column_name.clone());
                columns.push(ColumnInfo { name: column_name, data_type: "".into(), is_nullable: true, default_value: None });
            }
            tables.push(TableInfo { name: constraint_name, schema: owner, columns });
        }

        // Include sequences
        let seq_rows = conn
            .query(
                "SELECT sequence_owner, sequence_name, min_value, max_value, increment_by, cycle_flag, cache_size, order_flag, last_number 
                 FROM all_sequences 
                 ORDER BY sequence_owner, sequence_name",
                &[],
            )
            .map_err(|e| Error::Any(anyhow::anyhow!(e)))?;
        for row_res in seq_rows {
            let row = row_res.map_err(|e| Error::Any(anyhow::anyhow!(e)))?;
            let owner: String = row.get(0).map_err(|e| Error::Any(anyhow::anyhow!(e)))?;
            let seq_name: String = row.get(1).map_err(|e| Error::Any(anyhow::anyhow!(e)))?;
            let min_value: i64 = row.get(2).map_err(|e| Error::Any(anyhow::anyhow!(e)))?;
            let max_value: i64 = row.get(3).map_err(|e| Error::Any(anyhow::anyhow!(e)))?;
            let increment_by: i64 = row.get(4).map_err(|e| Error::Any(anyhow::anyhow!(e)))?;
            let cycle_flag: String = row.get(5).map_err(|e| Error::Any(anyhow::anyhow!(e)))?;
            let cache_size: i64 = row.get(6).map_err(|e| Error::Any(anyhow::anyhow!(e)))?;
            let order_flag: String = row.get(7).map_err(|e| Error::Any(anyhow::anyhow!(e)))?;
            let last_number: i64 = row.get(8).map_err(|e| Error::Any(anyhow::anyhow!(e)))?;
            schemas_set.insert(owner.clone());
            let columns = vec![
                ColumnInfo { name: "NEXTVAL".into(), data_type: "NUMBER".into(), is_nullable: false, default_value: None },
                ColumnInfo { name: "__SEQ_MIN_VALUE__".into(), data_type: min_value.to_string(), is_nullable: false, default_value: None },
                ColumnInfo { name: "__SEQ_MAX_VALUE__".into(), data_type: max_value.to_string(), is_nullable: false, default_value: None },
                ColumnInfo { name: "__SEQ_INCREMENT_BY__".into(), data_type: increment_by.to_string(), is_nullable: false, default_value: None },
                ColumnInfo { name: "__SEQ_CYCLE__".into(), data_type: cycle_flag, is_nullable: false, default_value: None },
                ColumnInfo { name: "__SEQ_CACHE_SIZE__".into(), data_type: cache_size.to_string(), is_nullable: false, default_value: None },
                ColumnInfo { name: "__SEQ_ORDER__".into(), data_type: order_flag, is_nullable: false, default_value: None },
                ColumnInfo { name: "__SEQ_LAST_NUMBER__".into(), data_type: last_number.to_string(), is_nullable: false, default_value: None },
            ];
            tables.push(TableInfo { name: seq_name, schema: owner, columns });
        }

        // Include program units (procedures/functions)
        let proc_rows = conn
            .query("SELECT owner, object_name, object_type FROM all_objects WHERE object_type IN ('FUNCTION','PROCEDURE') AND owner NOT IN ('SYS','SYSTEM') ORDER BY owner, object_name", &[])
            .map_err(|e| Error::Any(anyhow::anyhow!(e)))?;
        for row_res in proc_rows {
            let row = row_res.map_err(|e| Error::Any(anyhow::anyhow!(e)))?;
            let owner: String = row.get(0).map_err(|e| Error::Any(anyhow::anyhow!(e)))?;
            let name: String = row.get(1).map_err(|e| Error::Any(anyhow::anyhow!(e)))?;
            let obj_type: String = row.get(2).map_err(|e| Error::Any(anyhow::anyhow!(e)))?;
            schemas_set.insert(owner.clone());
            // add arguments from ALL_ARGUMENTS
            let mut columns = Vec::new();
            columns.push(ColumnInfo { name: "__OBJECT_TYPE__".into(), data_type: obj_type, is_nullable: false, default_value: None });
            let arg_rows = conn
                .query("SELECT argument_name, data_type, in_out, position, defaulted, default_value FROM all_arguments WHERE owner = :1 AND object_name = :2 ORDER BY position",
                       &[&owner, &name])
                .map_err(|e| Error::Any(anyhow::anyhow!(e)))?;
            let mut has_out = false;
            for arg_res in arg_rows {
                let arg = arg_res.map_err(|e| Error::Any(anyhow::anyhow!(e)))?;
                let arg_name: Option<String> = arg.get(0).map_err(|e| Error::Any(anyhow::anyhow!(e)))?;
                let dt: Option<String> = arg.get(1).map_err(|e| Error::Any(anyhow::anyhow!(e)))?;
                let mode: Option<String> = arg.get(2).map_err(|e| Error::Any(anyhow::anyhow!(e)))?;
                let position: Option<i64> = arg.get(3).map_err(|e| Error::Any(anyhow::anyhow!(e)))?;
                let defaulted: Option<String> = arg.get(4).map_err(|e| Error::Any(anyhow::anyhow!(e)))?;
                let default_val: Option<String> = arg.get(5).map_err(|e| Error::Any(anyhow::anyhow!(e)))?;
                if let Some(m) = &mode {
                    let m_up = m.to_uppercase();
                    if m_up == "OUT" || m_up == "IN/OUT" || m_up == "IN OUT" { has_out = true; }
                }
                let disp = match (arg_name, mode, position) {
                    (Some(n), Some(m), Some(p)) => format!("{} [{} p{}]", n, m, p),
                    (Some(n), Some(m), None) => format!("{} [{}]", n, m),
                    (Some(n), None, Some(p)) => format!("{} [p{}]", n, p),
                    (Some(n), None, None) => n,
                    (None, Some(m), Some(p)) => format!("<anonymous> [{} p{}]", m, p),
                    (None, Some(m), None) => format!("<anonymous> [{}]", m),
                    (None, None, Some(p)) => format!("<anonymous> [p{}]", p),
                    (None, None, None) => "<anonymous>".into(),
                };
                let def = match (defaulted.as_deref(), default_val) {
                    (Some("Y"), Some(v)) => Some(v),
                    (Some("Y"), None) => Some("<default>".into()),
                    _ => None,
                };
                columns.push(ColumnInfo { name: disp, data_type: dt.unwrap_or_default(), is_nullable: true, default_value: def });
            }
            columns.push(ColumnInfo { name: "__HAS_OUT_ARGS__".into(), data_type: if has_out { "YES".into() } else { "NO".into() }, is_nullable: false, default_value: None });
            tables.push(TableInfo { name, schema: owner, columns });
        }

        // Include synonyms
        let syn_rows = conn
            .query(
                "SELECT owner, synonym_name, table_owner, table_name, db_link FROM all_synonyms WHERE owner NOT IN ('SYS','SYSTEM') ORDER BY owner, synonym_name",
                &[],
            )
            .map_err(|e| Error::Any(anyhow::anyhow!(e)))?;
        for row_res in syn_rows {
            let row = row_res.map_err(|e| Error::Any(anyhow::anyhow!(e)))?;
            let owner: String = row.get(0).map_err(|e| Error::Any(anyhow::anyhow!(e)))?;
            let syn_name: String = row.get(1).map_err(|e| Error::Any(anyhow::anyhow!(e)))?;
            let tgt_owner: Option<String> = row.get(2).map_err(|e| Error::Any(anyhow::anyhow!(e)))?;
            let tgt_name: Option<String> = row.get(3).map_err(|e| Error::Any(anyhow::anyhow!(e)))?;
            let db_link: Option<String> = row.get(4).map_err(|e| Error::Any(anyhow::anyhow!(e)))?;
            schemas_set.insert(owner.clone());
            let mut columns = Vec::new();
            if let (Some(o), Some(n)) = (tgt_owner.clone(), tgt_name.clone()) {
                columns.push(ColumnInfo { name: "__SYNONYM_TARGET__".into(), data_type: format!("{}.{}", o, n), is_nullable: false, default_value: None });
                // resolution status
                let exists = match conn.query("SELECT COUNT(*) FROM all_objects WHERE owner = :1 AND object_name = :2", &[&o, &n]) {
                    Ok(mut q) => {
                        match q.next() {
                            Some(Ok(r)) => r.get::<usize, i64>(0).map(|c| c > 0).unwrap_or(false),
                            _ => false,
                        }
                    }
                    Err(_) => false,
                };
                // if db_link present, mark as REMOTE
                if db_link.is_some() {
                    columns.push(ColumnInfo { name: "__SYNONYM_RESOLVED__".into(), data_type: "REMOTE".into(), is_nullable: false, default_value: None });
                    // Check local presence of DB_LINK
                    if let Some(link) = db_link.clone() {
                        let present = match conn.query("SELECT COUNT(*) FROM all_db_links WHERE db_link = :1", &[&link]) {
                            Ok(mut q) => match q.next() { Some(Ok(r)) => r.get::<usize, i64>(0).map(|c| c > 0).unwrap_or(false), _ => false },
                            Err(_) => false,
                        };
                        columns.push(ColumnInfo { name: "__SYNONYM_DB_LINK_STATUS__".into(), data_type: if present { "PRESENT".into() } else { "MISSING".into() }, is_nullable: false, default_value: None });

                        // Optional remote ping: guarded by env ORACLE_ALLOW_DB_LINK_PING
                        let allow = std::env::var("ORACLE_ALLOW_DB_LINK_PING").ok().map(|v| v.eq_ignore_ascii_case("1") || v.eq_ignore_ascii_case("true")).unwrap_or(false);
                        if allow {
                            let sql = format!("SELECT 1 FROM DUAL@{}", link);
                            let ping_ok = match conn.query(&sql, &[]) {
                                Ok(mut q) => matches!(q.next(), Some(Ok(_))),
                                Err(_) => false,
                            };
                            columns.push(ColumnInfo { name: "__SYNONYM_DB_LINK_PING__".into(), data_type: if ping_ok { "OK".into() } else { "FAIL".into() }, is_nullable: false, default_value: None });
                        }
                    }
                } else {
                    columns.push(ColumnInfo { name: "__SYNONYM_RESOLVED__".into(), data_type: if exists { "RESOLVED".into() } else { "BROKEN".into() }, is_nullable: false, default_value: None });
                }
            }
            if let Some(link) = db_link { columns.push(ColumnInfo { name: "__SYNONYM_DB_LINK__".into(), data_type: link, is_nullable: false, default_value: None }); }
            tables.push(TableInfo { name: syn_name, schema: owner, columns });
        }

        // Include triggers
        let trg_rows = conn
            .query(
                "SELECT owner, trigger_name, table_owner, table_name, status, triggering_event, trigger_type 
                 FROM all_triggers WHERE owner NOT IN ('SYS','SYSTEM') ORDER BY owner, trigger_name",
                &[],
            )
            .map_err(|e| Error::Any(anyhow::anyhow!(e)))?;
        for row_res in trg_rows {
            let row = row_res.map_err(|e| Error::Any(anyhow::anyhow!(e)))?;
            let owner: String = row.get(0).map_err(|e| Error::Any(anyhow::anyhow!(e)))?;
            let trg_name: String = row.get(1).map_err(|e| Error::Any(anyhow::anyhow!(e)))?;
            let tbl_owner: Option<String> = row.get(2).map_err(|e| Error::Any(anyhow::anyhow!(e)))?;
            let tbl_name: Option<String> = row.get(3).map_err(|e| Error::Any(anyhow::anyhow!(e)))?;
            let status: String = row.get(4).map_err(|e| Error::Any(anyhow::anyhow!(e)))?;
            let evt: String = row.get(5).map_err(|e| Error::Any(anyhow::anyhow!(e)))?;
            let trg_type: String = row.get(6).map_err(|e| Error::Any(anyhow::anyhow!(e)))?;
            schemas_set.insert(owner.clone());
            let mut columns = Vec::new();
            if let (Some(o), Some(n)) = (tbl_owner, tbl_name) {
                columns.push(ColumnInfo { name: "__TRIGGER_TABLE__".into(), data_type: format!("{}.{}", o, n), is_nullable: false, default_value: None });
            }
            columns.push(ColumnInfo { name: "__TRIGGER_STATUS__".into(), data_type: status, is_nullable: false, default_value: None });
            columns.push(ColumnInfo { name: "__TRIGGER_EVENT__".into(), data_type: evt, is_nullable: false, default_value: None });
            columns.push(ColumnInfo { name: "__TRIGGER_TYPE__".into(), data_type: trg_type, is_nullable: false, default_value: None });
            // optional firing order if column exists
            if let Ok(mut q) = conn.query("SELECT firing_order FROM all_triggers WHERE owner = :1 AND trigger_name = :2", &[&owner, &trg_name]) {
                if let Some(Ok(r)) = q.next() {
                    if let Ok(ord) = r.get::<usize, i64>(0) {
                        columns.push(ColumnInfo { name: "__TRIGGER_ORDER__".into(), data_type: ord.to_string(), is_nullable: false, default_value: None });
                    }
                }
            }
            tables.push(TableInfo { name: trg_name, schema: owner, columns });
        }

        // Deeper table partition metadata: summarize partitions and subpartitions per table
        use std::collections::HashMap;
        let mut table_index: HashMap<(String, String), usize> = HashMap::new();
        for (i, t) in tables.iter().enumerate() { table_index.insert((t.schema.clone(), t.name.clone()), i); }
        let tpart_rows = conn
            .query("SELECT table_owner, table_name, COUNT(*) FROM all_tab_partitions GROUP BY table_owner, table_name", &[])
            .map_err(|e| Error::Any(anyhow::anyhow!(e)))?;
        for row_res in tpart_rows {
            let row = row_res.map_err(|e| Error::Any(anyhow::anyhow!(e)))?;
            let owner: String = row.get(0).map_err(|e| Error::Any(anyhow::anyhow!(e)))?;
            let tname: String = row.get(1).map_err(|e| Error::Any(anyhow::anyhow!(e)))?;
            let count: i64 = row.get(2).map_err(|e| Error::Any(anyhow::anyhow!(e)))?;
            if let Some(idx) = table_index.get(&(owner.clone(), tname.clone())) {
                if let Some(tab) = tables.get_mut(*idx) {
                    tab.columns.push(ColumnInfo { name: "__TABLE_PARTITIONS__".into(), data_type: count.to_string(), is_nullable: false, default_value: None });
                }
            }
        }
        let subpart_rows = conn
            .query("SELECT table_owner, table_name, COUNT(*) FROM all_tab_subpartitions GROUP BY table_owner, table_name", &[])
            .map_err(|e| Error::Any(anyhow::anyhow!(e)))?;
        for row_res in subpart_rows {
            let row = row_res.map_err(|e| Error::Any(anyhow::anyhow!(e)))?;
            let owner: String = row.get(0).map_err(|e| Error::Any(anyhow::anyhow!(e)))?;
            let tname: String = row.get(1).map_err(|e| Error::Any(anyhow::anyhow!(e)))?;
            let count: i64 = row.get(2).map_err(|e| Error::Any(anyhow::anyhow!(e)))?;
            if let Some(idx) = table_index.get(&(owner.clone(), tname.clone())) {
                if let Some(tab) = tables.get_mut(*idx) {
                    tab.columns.push(ColumnInfo { name: "__TABLE_SUBPARTITIONS__".into(), data_type: count.to_string(), is_nullable: false, default_value: None });
                }
            }
        }

        // Table comments
        let comments_rows = conn
            .query("SELECT owner, table_name, comments FROM all_tab_comments WHERE owner NOT IN ('SYS','SYSTEM') AND comments IS NOT NULL", &[])
            .map_err(|e| Error::Any(anyhow::anyhow!(e)))?;
        for row_res in comments_rows {
            let row = row_res.map_err(|e| Error::Any(anyhow::anyhow!(e)))?;
            let owner: String = row.get(0).map_err(|e| Error::Any(anyhow::anyhow!(e)))?;
            let tname: String = row.get(1).map_err(|e| Error::Any(anyhow::anyhow!(e)))?;
            let comment: Option<String> = row.get(2).ok();
            if let (Some(idx), Some(c)) = (table_index.get(&(owner.clone(), tname.clone())), comment) {
                if let Some(tab) = tables.get_mut(*idx) {
                    tab.columns.push(ColumnInfo { name: "__TABLE_COMMENT__".into(), data_type: "".into(), is_nullable: false, default_value: Some(c) });
                }
            }
        }

        // Partition key columns summary
        use std::collections::BTreeMap;
        let mut part_keys: BTreeMap<(String, String), Vec<String>> = BTreeMap::new();
        let pk_rows = conn
            .query("SELECT owner, name, column_name FROM all_part_key_columns WHERE owner NOT IN ('SYS','SYSTEM') ORDER BY owner, name, column_position", &[])
            .map_err(|e| Error::Any(anyhow::anyhow!(e)))?;
        for row_res in pk_rows {
            let row = row_res.map_err(|e| Error::Any(anyhow::anyhow!(e)))?;
            let owner: String = row.get(0).map_err(|e| Error::Any(anyhow::anyhow!(e)))?;
            let tname: String = row.get(1).map_err(|e| Error::Any(anyhow::anyhow!(e)))?;
            let col: String = row.get(2).map_err(|e| Error::Any(anyhow::anyhow!(e)))?;
            part_keys.entry((owner, tname)).or_default().push(col);
        }
        for ((owner, tname), cols) in part_keys {
            if let Some(idx) = table_index.get(&(owner.clone(), tname.clone())) {
                if let Some(tab) = tables.get_mut(*idx) {
                    tab.columns.push(ColumnInfo { name: "__PARTITION_KEY__".into(), data_type: cols.join(","), is_nullable: false, default_value: None });
                }
            }
        }

        // Subpartition key columns summary
        let mut subpart_keys: BTreeMap<(String, String), Vec<String>> = BTreeMap::new();
        let spk_rows = conn
            .query("SELECT owner, name, column_name FROM all_subpart_key_columns WHERE owner NOT IN ('SYS','SYSTEM') ORDER BY owner, name, column_position", &[])
            .map_err(|e| Error::Any(anyhow::anyhow!(e)))?;
        for row_res in spk_rows {
            let row = row_res.map_err(|e| Error::Any(anyhow::anyhow!(e)))?;
            let owner: String = row.get(0).map_err(|e| Error::Any(anyhow::anyhow!(e)))?;
            let tname: String = row.get(1).map_err(|e| Error::Any(anyhow::anyhow!(e)))?;
            let col: String = row.get(2).map_err(|e| Error::Any(anyhow::anyhow!(e)))?;
            subpart_keys.entry((owner, tname)).or_default().push(col);
        }
        for ((owner, tname), cols) in subpart_keys {
            if let Some(idx) = table_index.get(&(owner.clone(), tname.clone())) {
                if let Some(tab) = tables.get_mut(*idx) {
                    tab.columns.push(ColumnInfo { name: "__SUBPARTITION_KEY__".into(), data_type: cols.join(","), is_nullable: false, default_value: None });
                }
            }
        }

        // Privileges summary per table
        let mut privs: BTreeMap<(String, String), Vec<String>> = BTreeMap::new();
        let priv_rows = conn
            .query("SELECT owner, table_name, grantee, privilege FROM all_tab_privs WHERE owner NOT IN ('SYS','SYSTEM') ORDER BY owner, table_name", &[])
            .map_err(|e| Error::Any(anyhow::anyhow!(e)))?;
        for row_res in priv_rows {
            let row = row_res.map_err(|e| Error::Any(anyhow::anyhow!(e)))?;
            let owner: String = row.get(0).map_err(|e| Error::Any(anyhow::anyhow!(e)))?;
            let tname: String = row.get(1).map_err(|e| Error::Any(anyhow::anyhow!(e)))?;
            let grantee: String = row.get(2).map_err(|e| Error::Any(anyhow::anyhow!(e)))?;
            let privilege: String = row.get(3).map_err(|e| Error::Any(anyhow::anyhow!(e)))?;
            privs.entry((owner, tname)).or_default().push(format!("{}:{}", grantee, privilege));
        }
        for ((owner, tname), entries) in privs {
            if let Some(idx) = table_index.get(&(owner.clone(), tname.clone())) {
                if let Some(tab) = tables.get_mut(*idx) {
                    tab.columns.push(ColumnInfo { name: "__PRIVS__".into(), data_type: entries.join(", "), is_nullable: false, default_value: None });
                }
            }
        }

        // Package grouping: summarize members under package as a separate entry
        let pkg_rows = conn
            .query("SELECT owner, object_name, procedure_name FROM all_procedures WHERE owner NOT IN ('SYS','SYSTEM') ORDER BY owner, object_name, procedure_name", &[])
            .map_err(|e| Error::Any(anyhow::anyhow!(e)))?;
        let mut pkg_members: BTreeMap<(String, String), Vec<String>> = BTreeMap::new();
        for row_res in pkg_rows {
            let row = row_res.map_err(|e| Error::Any(anyhow::anyhow!(e)))?;
            let owner: String = row.get(0).map_err(|e| Error::Any(anyhow::anyhow!(e)))?;
            let pkg: String = row.get(1).map_err(|e| Error::Any(anyhow::anyhow!(e)))?;
            let member: Option<String> = row.get(2).ok();
            if let Some(m) = member { pkg_members.entry((owner, pkg)).or_default().push(m); }
        }
        for ((owner, pkg), members) in pkg_members {
            let mut cols = Vec::new();
            cols.push(ColumnInfo { name: "__PACKAGE_MEMBER_COUNT__".into(), data_type: members.len().to_string(), is_nullable: false, default_value: None });
            for m in members { cols.push(ColumnInfo { name: "__PACKAGE_MEMBER__".into(), data_type: m, is_nullable: false, default_value: None }); }
            tables.push(TableInfo { name: pkg, schema: owner, columns: cols });
        }

        fn describe_constraint(code: &str) -> String {
            match code {
                "C" => "Check".into(),
                "P" => "Primary Key".into(),
                "U" => "Unique".into(),
                "R" => "Foreign Key".into(),
                "V" => "With Check Option".into(),
                "O" => "With Read Only".into(),
                "H" => "Hash".into(),
                "S" => "Supplemental".into(),
                other => other.into(),
            }
        }

        let unique_columns = unique_columns_set.into_iter().collect();
        let schemas = schemas_set.into_iter().collect();
        Ok(DatabaseSchema { tables, schemas, unique_columns }) as Result<_, Error>
    }).await?
}
