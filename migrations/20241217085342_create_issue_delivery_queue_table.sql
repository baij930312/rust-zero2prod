-- Add migration script here
CREATE TABLE
    issues_delivery_queue (
        newsletter_issues_id uuid NOT NULL REFERENCES newsletter_issues (newsletter_issues_id),
        subscriber_email TEXT NOT NULL,
        PRIMARY KEY (newsletter_issues_id, subscriber_email)
    )