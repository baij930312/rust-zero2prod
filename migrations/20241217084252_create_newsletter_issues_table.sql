-- Add migration script here
CREATE TABLE
    newsletter_issues (
        newsletter_issues_id uuid NOT NULL,
        title TEXT NOT NULL,
        text_content TEXT NOT NULL,
        html_content TEXT NOT NULL,
        published_at timestamptz NOT NULL,
        PRIMARY KEY(newsletter_issues_id) 
    )