import hashlib
import json
from collections import defaultdict

from defaultlist import defaultlist
from telegram import (
    ReplyKeyboardMarkup,
    KeyboardButton,
    InlineKeyboardMarkup,
    InlineKeyboardButton,
)

import config
from telegram.ext import (
    Updater,
    CommandHandler,
    MessageHandler,
    Filters,
    CallbackQueryHandler,
)
import logging
import requests
from datetime import datetime, timedelta, timezone

logging.basicConfig(
    format="%(asctime)s - %(name)s - %(levelname)s - %(message)s", level=logging.INFO
)
logger = logging.getLogger(__name__)
updater = Updater(token=config.token, use_context=True)
dispatcher = updater.dispatcher


def get_bookings():
    response = requests.get("{}/bookings".format(config.booker_api))
    bookings = json.loads(response.content)
    print(bookings)
    return bookings


def get_bookings_md(bookings):
    if len(bookings) == 0:
        text = "no bookings found"
    else:
        text = ""
        for booking in bookings:
            text += "{} at {} |   {}  \n".format(
                datetime.strptime(booking["date"], "%d/%m/%Y").strftime("%a %d/%m"),
                booking["court_time"],
                booking["court_number"],
            )
    return text


def bookings(update, context):
    context.bot.send_message(
        chat_id=update.message.chat_id,
        text="""
<pre>
     Booking       | Court #
 ----------------- | --------
"""
        + get_bookings_md(get_bookings())
        + """
</pre>
""",
        parse_mode="html",
    )


bookings_handler = CommandHandler("bookings", bookings)
dispatcher.add_handler(bookings_handler)


def bots(update, context):
    response = requests.get("{}/bots".format(config.booker_api))
    bots = json.loads(response.content)
    if len(bots) == 0:
        text = "no bots found"
    else:
        text = ""
        for bot in bots:
            text += "{}: {} at {} ({})\n".format(
                bot["name"], bot["week_day"], bot["court_time"], bot["status"]
            )

    context.bot.send_message(chat_id=update.message.chat_id, text=text)


bots_handler = CommandHandler("bots", bots)
dispatcher.add_handler(bots_handler)


def handle_response(bot, response, chat_id, usage):
    print("response code = {}".format(response.status_code))
    status = (
        "ok"
        if response.status_code >= 200 and response.status_code < 300
        else "ko, usage: \n{}".format(usage)
    )
    bot.send_message(chat_id=chat_id, text=status)


def deploy(update, context):
    usage = "/deploy"
    response = requests.post("{}/bots/actions/deploy".format(config.booker_api))
    handle_response(context.bot, response, update.message.chat_id, usage)


deploy_handler = CommandHandler("deploy", deploy)
dispatcher.add_handler(deploy_handler)


def add(update, context):
    usage = "/add [day] [court_time] (ex: /add monday 19:20)"
    logger.info(context.args)
    if len(context.args) != 2:
        context.bot.send_message(
            chat_id=update.message.chat_id, text="ko: usage: \n{}".format(usage)
        )
        return
    day = context.args[0]
    court_time = context.args[1]
    payload = {
        "name": "bot_{}_{}".format(day, court_time).replace(":", "_"),
        "week_day": day[0].upper() + day[1:],
        "court_time": court_time,
        "status": "Created",
    }
    print(payload)
    response = requests.post("{}/bots".format(config.booker_api), json=payload)
    handle_response(context.bot, response, update.message.chat_id, usage)


add_handler = CommandHandler("add", add)
dispatcher.add_handler(add_handler)


def delete(update, context):
    usage = "/delete [bot_name] (ex: /delete bot_monday_19_20)"
    logger.info(context.args)
    if len(context.args) != 1:
        context.bot.send_message(
            chat_id=update.message.chat_id, text="ko: usage: \n{}".format(usage)
        )
        return
    bot = context.args[0]
    response = requests.delete("{}/bots/{}".format(config.booker_api, bot))
    handle_response(context.bot, response, update.message.chat_id, usage)


delete_handler = CommandHandler("delete", delete)
dispatcher.add_handler(delete_handler)


def cancel(update, context):
    usage = "/cancel [booking_number] (ex: /cancel 1)"
    logger.info(context.args)
    if len(context.args) != 1:
        context.bot.send_message(
            chat_id=update.message.chat_id, text="ko: usage: \n{}".format(usage)
        )
        return
    idx = context.args[0]
    bookings = get_bookings()
    booking_id = bookings[int(idx) - 1]["id"]
    response = requests.delete("{}/bookings/{}".format(config.booker_api, booking_id))
    handle_response(context.bot, response, update.message.chat_id, usage)


class InlineKeyboardFormatter:
    ROW_MAX_LENGTH = 44

    def __init__(self):
        self.inline_keyboard = defaultlist(list)
        self.current_row = 0
        self.current_row_length = 0

    def add_ik_button(self, text, data):
        row_new_idx = self.current_row_length + len(text)
        print(
            f"start current_row={self.current_row} / current_row_length={self.current_row_length} / row_new_idx={row_new_idx}"
        )
        if row_new_idx > self.ROW_MAX_LENGTH:
            self.current_row += 1
            self.current_row_length = len(text)
        else:
            self.current_row_length = row_new_idx
        print(
            f"add current_row={self.current_row} / current_row_length={self.current_row_length}"
        )
        self.inline_keyboard[self.current_row].append(
            InlineKeyboardButton(text, callback_data=json.dumps(data))
        )


cancel_handler = CommandHandler("cancel", cancel)
dispatcher.add_handler(cancel_handler)


def accept_dialog(update, context):
    ik_formatter = InlineKeyboardFormatter()
    bookings = get_bookings()
    bookings_by_day = defaultdict(list)
    for idx, booking in enumerate(bookings):
        bookings_by_day[booking["date"]].append(booking)
    print(len(bookings_by_day))
    for date, day_bookings in bookings_by_day.items():
        booking_date = datetime.strptime(date, "%d/%m/%Y").strftime("%a %d")
        start = min(day_bookings, key=lambda dict: dict["court_time"])["court_time"]
        end = (
            datetime.strptime(
                max(day_bookings, key=lambda dict: dict["court_time"])["court_time"],
                "%H:%M",
            )
            + timedelta(minutes=40)
        ).strftime("%H:%M")
        ik_formatter.add_ik_button(
            "{} {}->{}".format(booking_date, start, end),
            {
                "action": "accept",
                "bookings": [booking["id"] for booking in day_bookings],
            },
        )

    context.bot.send_message(
        chat_id=update.message.chat_id,
        text="chose a court period to get invite",
        reply_markup=InlineKeyboardMarkup(inline_keyboard=ik_formatter.inline_keyboard),
    )


def accept_callback(bot, chat_id, ids):
    usage = "/accept [court_1] [[court_2]...[court_n]] (ex: /accept 1 2)"
    bookings = [booking for booking in get_bookings() if booking["id"] in ids]
    try:
        logger.info(bookings)
        start = min(bookings, key=lambda dict: dict["court_time"])
        end = max(bookings, key=lambda dict: dict["court_time"])
        start = datetime.strptime(
            "{} {}".format(start["date"], start["court_time"]), "%d/%m/%Y %H:%M"
        )
        end = datetime.strptime(
            "{} {}".format(end["date"], end["court_time"]), "%d/%m/%Y %H:%M"
        ) + timedelta(minutes=40)
        with open("invite.squash.ics.template", "r") as template_handle, open(
            "invite.squash.ics", "w"
        ) as to_send_handle:
            start_str = start.replace(tzinfo=timezone.utc).strftime("%Y%m%dT%H%M%S")
            end_str = end.replace(tzinfo=timezone.utc).strftime("%Y%m%dT%H%M%S")
            id = hashlib.md5(
                "{}-{}".format(start_str, end_str).encode("utf-8")
            ).hexdigest()
            data = (
                template_handle.read()
                .replace("{{start}}", start_str)
                .replace("{{end}}", end_str)
                .replace("{{id}}", id)
            )
            to_send_handle.write(data)
        response = requests.post(
            "https://api.telegram.org/bot{}/sendDocument".format(config.token),
            files={"document": open("invite.squash.ics", "rb")},
            data={"chat_id": chat_id},
        )
        handle_response(bot, response, chat_id, usage)
    except IndexError:
        bot.send_message(
            chat_id=chat_id, text="wrong court numbers, usage: \n{}".format(usage)
        )


accept_handler = CommandHandler("accept", accept_dialog)
dispatcher.add_handler(accept_handler)


def echo(update, context):
    logger.info('echo "{}"'.format(update.message.text))
    context.bot.send_message(chat_id=update.message.chat_id, text=update.message.text)


echo_handler = MessageHandler(Filters.text, echo)
dispatcher.add_handler(echo_handler)


def callback_manager(update, callback_context):
    data = json.loads(update.callback_query["data"])
    if data["action"] == "accept":
        accept_callback(
            callback_context.bot,
            update.callback_query.message.chat.id,
            data["bookings"],
        )


callback_query_handler = CallbackQueryHandler(callback_manager)
dispatcher.add_handler(callback_query_handler)


def help(update, context):
    logger.info("chat_id={}".format(update.message.chat_id))
    help_text = """
    commands availables:
    /accept -> accept court(s) attending
    /add [day] [court_time] -> create a bot for day of week [day] at [court_time]  
    /bookings -> display all bookings  
    /bots -> display all bots and their statuses
    /cancel [booking_number] -> cancel the booking #[booking_number]
    /delete [bot_name] -> delete the bot [bot_name]  
    /deploy -> start all the created bots  
    /help -> display this message
    """
    context.bot.send_message(chat_id=update.message.chat_id, text=help_text)


help_handler = CommandHandler("help", help)
dispatcher.add_handler(help_handler)


def unknown(update, context):
    context.bot.send_message(
        chat_id=update.message.chat_id, text="Sorry, I didn't understand that command."
    )


unknown_handler = MessageHandler(Filters.command, unknown)
dispatcher.add_handler(unknown_handler)

updater.start_polling()
