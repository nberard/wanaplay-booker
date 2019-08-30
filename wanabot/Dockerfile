FROM python:3.6.5-slim


COPY requirements.txt /opt
RUN pip install --upgrade pip
RUN pip install --no-cache-dir -r /opt/requirements.txt

COPY . /opt
WORKDIR /opt
CMD python bot.py