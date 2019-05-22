FROM python:3.6.5-slim


COPY requirements.txt /opt
COPY python_telegram_bot/requirements.txt /opt/requirements_telegram.txt

RUN apt-get update \
    && pip install --no-cache-dir -r /opt/requirements.txt \
    && pip install --no-cache-dir -r /opt/requirements_telegram.txt \
    && apt-get autoremove -y \
    && rm -rf /var/lib/apt/lists/*

COPY python_telegram_bot /opt/
COPY config.py.dist /opt/config.py
COPY invite.squash.ics.template /opt/
COPY bot.py /opt/
WORKDIR /opt
CMD python bot.py