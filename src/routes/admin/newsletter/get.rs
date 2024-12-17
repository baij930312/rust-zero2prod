use ::actix_web::HttpResponse;
use actix_web::http::header::ContentType;
use actix_web_flash_messages::IncomingFlashMessages;
use std::fmt::Write;

pub async fn newsletters_form(flash_messages: IncomingFlashMessages) -> HttpResponse {
    let mut error_html = String::new();
    let idempotency_key = uuid::Uuid::new_v4().to_string();
    for m in flash_messages.iter() {
        writeln!(error_html, "<p><i>{}</i></p>", m.content()).unwrap();
    }

    let response = HttpResponse::Ok()
        .content_type(ContentType::html())
        .body(format!(
            r#"
        <!DOCTYPE html>
<html lang="en">
 
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Login</title>
</head>

<body>
        {error_html}
    <form action="/admin/newsletters" method="post">
        <label>Title
            <input type="text" placeholder="Enter Title" name="title" />
        </label>
 
        <label>Content
            <input type="text" placeholder="Enter content" name="text_content" />
        </label>

            <label>Html
            <input type="text" placeholder="Enter Html" name="html_content" />
        </label> 
           <input hidden type="text"   name="idempotency_key" value="{idempotency_key}" />
 
        <button type="submit">submit</button>
    </form>  

</body>

</html>
        "#
        ));

    response
}
